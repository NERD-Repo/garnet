// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "host_device.h"

#include <zircon/status.h>

#include "garnet/drivers/bluetooth/lib/common/log.h"
#include "garnet/drivers/bluetooth/lib/hci/device_wrapper.h"
#include "garnet/lib/bluetooth/c/bt_host.h"

#include "host.h"

namespace bthost {

constexpr uint16_t kDeviceInformationServiceUuid = 0x180A;
constexpr uint16_t kBatteryServiceUuid = 0x180F;

HostDevice::HostDevice(zx_device_t* device)
    : device_created_(false),
      dev_(nullptr),
      parent_(device),
      loop_(&kAsyncLoopConfigNoAttachToThread),
      remote_service_loop_(&kAsyncLoopConfigNoAttachToThread) {
  FXL_DCHECK(parent_);

  dev_proto_.version = DEVICE_OPS_VERSION;
  dev_proto_.unbind = &HostDevice::DdkUnbind;
  dev_proto_.release = &HostDevice::DdkRelease;
  dev_proto_.ioctl = &HostDevice::DdkIoctl;
}

zx_status_t HostDevice::Bind() {
  bt_log(TRACE, "bt-host", "bind");

  std::lock_guard<std::mutex> lock(mtx_);

  bt_hci_protocol_t hci_proto;
  zx_status_t status =
      device_get_protocol(parent_, ZX_PROTOCOL_BT_HCI, &hci_proto);
  if (status != ZX_OK) {
    bt_log(ERROR, "bt-host", "failed to obtain bt-hci protocol ops: %s",
           zx_status_get_string(status));
    return status;
  }

  if (!hci_proto.ops) {
    bt_log(ERROR, "bt-host", "bt-hci device ops required!");
    return ZX_ERR_NOT_SUPPORTED;
  }

  if (!hci_proto.ops->open_command_channel) {
    bt_log(ERROR, "bt-host", "bt-hci op required: open_command_channel");
    return ZX_ERR_NOT_SUPPORTED;
  }

  if (!hci_proto.ops->open_acl_data_channel) {
    bt_log(ERROR, "bt-host", "bt-hci op required: open_acl_data_channel");
    return ZX_ERR_NOT_SUPPORTED;
  }

  if (!hci_proto.ops->open_snoop_channel) {
    bt_log(ERROR, "bt-host", "bt-hci op required: open_snoop_channel");
    return ZX_ERR_NOT_SUPPORTED;
  }

  // We are required to publish a device before returning from Bind but we
  // haven't fully initialized the adapter yet. We create the bt-host device as
  // invisible until initialization completes on the host thread. We also
  // disallow other drivers from directly binding to it.
  device_add_args_t args = {
      .version = DEVICE_ADD_ARGS_VERSION,
      .name = "bt-host",
      .ctx = this,
      .ops = &dev_proto_,
      .proto_id = ZX_PROTOCOL_BT_HOST,
      .flags = DEVICE_ADD_NON_BINDABLE | DEVICE_ADD_INVISIBLE,
  };

  status = device_add(parent_, &args, &dev_);
  if (status != ZX_OK) {
    bt_log(ERROR, "bt-host", "Failed to publish device: %s",
           zx_status_get_string(status));
    return status;
  }

  // When the device is initialized.
  device_created_ = true;

  status = loop_.StartThread("bt-host (gap)");
  if (status != ZX_OK) {
    bt_log(ERROR, "bt-host", "Failed to create host thread: %s",
           zx_status_get_string(status));
    CleanUp();
    return status;
  }

  for (int i = 0; i < kGattRemoteServiceDeviceDispatchThreads; i++) {
    status = remote_service_loop_.StartThread("bt-host bt-gatt-svc dispatcher");
    if (status != ZX_OK) {
      bt_log(ERROR, "bt-host", "Failed to create driver child thread: %s",
             zx_status_get_string(status));
      remote_service_loop_.Shutdown();
      loop_.Shutdown();
      CleanUp();
      return status;
    }
  }

  // Send the bootstrap message to Host. The Host object can only be accessed on
  // the Host thread.
  async::PostTask(loop_.dispatcher(), [hci_proto, this] {
    bt_log(SPEW, "bt-host", "host thread start");

    std::lock_guard<std::mutex> lock(mtx_);
    host_ = fxl::MakeRefCounted<Host>(hci_proto);
    host_->Initialize([host = host_, this](bool success) {
      {
        std::lock_guard<std::mutex> lock(mtx_);

        // Abort if CleanUp has been called.
        if (!host_)
          return;

        if (success) {
          bt_log(TRACE, "bt-host", "adapter initialized; make device visible");
          host_->gatt_host()->SetRemoteServiceWatcher(
              fit::bind_member(this, &HostDevice::OnRemoteGattServiceAdded));
          device_make_visible(dev_);
          return;
        }

        bt_log(ERROR, "bt-host", "failed to initialize adapter");
        CleanUp();
      }

      host->ShutDown();
      remote_service_loop_.Shutdown();
      loop_.Shutdown();
    });
  });

  return ZX_OK;
}

void HostDevice::Unbind() {
  bt_log(TRACE, "bt-host", "unbind");

  std::lock_guard<std::mutex> lock(mtx_);

  if (host_) {
    // Do this immediately to stop receiving new service callbacks.
    host_->gatt_host()->SetRemoteServiceWatcher({});
  }

  async::PostTask(loop_.dispatcher(), [this, host = host_] {
    if (host) {
      host->ShutDown();
    }
    loop_.Quit();
    remote_service_loop_.Quit();
  });

  // Make sure that the ShutDown task runs before this returns.
  remote_service_loop_.JoinThreads();
  loop_.JoinThreads();

  CleanUp();
}

void HostDevice::Release() {
  bt_log(TRACE, "bt-host", "release");
  delete this;
}

zx_status_t HostDevice::Ioctl(uint32_t op, const void* in_buf, size_t in_len,
                              void* out_buf, size_t out_len,
                              size_t* out_actual) {
  bt_log(TRACE, "bt-host", "ioctl");

  if (!out_buf)
    return ZX_ERR_INVALID_ARGS;

  if (out_len < sizeof(zx_handle_t))
    return ZX_ERR_BUFFER_TOO_SMALL;

  if (op != IOCTL_BT_HOST_OPEN_CHANNEL)
    return ZX_ERR_NOT_SUPPORTED;

  zx::channel local, remote;
  zx_status_t status = zx::channel::create(0, &local, &remote);
  if (status != ZX_OK)
    return status;

  ZX_DEBUG_ASSERT(local);
  ZX_DEBUG_ASSERT(remote);

  std::lock_guard<std::mutex> lock(mtx_);

  // Tell Host to start processing messages on this handle.
  ZX_DEBUG_ASSERT(host_);
  async::PostTask(loop_.dispatcher(),
                  [host = host_, chan = std::move(local)]() mutable {
                    host->BindHostInterface(std::move(chan));
                  });

  zx_handle_t* reply = static_cast<zx_handle_t*>(out_buf);
  *reply = remote.release();
  *out_actual = sizeof(zx_handle_t);

  return ZX_OK;
}

void HostDevice::OnRemoteGattServiceAdded(
    const std::string& peer_id,
    fbl::RefPtr<btlib::gatt::RemoteService> service) {
  // Battery and device information services are special case. We don't allow
  // drivers to be bound to them.
  if (service->uuid() == kDeviceInformationServiceUuid ||
      service->uuid() == kBatteryServiceUuid) {
    return;
  }

  std::lock_guard<std::mutex> lock(mtx_);
  fxl::RefPtr<GattRemoteServiceDevice> gatt_device =
      fxl::MakeRefCounted<GattRemoteServiceDevice>(
          dev_, peer_id, remote_service_loop_.dispatcher(), service);

  service->AddRemovedHandler(
      [this, gatt_ref = gatt_device] {
        gatt_devices_.erase(gatt_ref);
        async::PostTask(remote_service_loop_.dispatcher(),
                        [gatt_ref] { gatt_ref->Shutdown(); });
      },
      loop_.dispatcher());

  zx_status_t status = gatt_device->Bind();
  if (status != ZX_OK) {
    bt_log(ERROR, "bt-host", "Unable to create gatt child device: %s",
           zx_status_get_string(status));
    return;
  }

  gatt_devices_.insert(std::move(gatt_device));
}

void HostDevice::CleanUp() {
  host_ = nullptr;

  // Removing the devices explictly instead of letting unbind handle it for us.
  for (fxl::RefPtr<GattRemoteServiceDevice> gatt_device : gatt_devices_) {
    if (gatt_device) {
      gatt_device->Shutdown();
    }
  }
  gatt_devices_.clear();
  if (device_created_)
    device_remove(dev_);

  dev_ = nullptr;
}

}  // namespace bthost
