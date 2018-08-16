// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef GARNET_DRIVERS_BLUETOOTH_HOST_GATT_REMOTE_SERVICE_DEVICE_H_
#define GARNET_DRIVERS_BLUETOOTH_HOST_GATT_REMOTE_SERVICE_DEVICE_H_

#include <zircon/types.h>

#include <ddk/device.h>
#include <ddk/driver.h>
#include <ddk/protocol/bt-gatt-svc.h>

#include <lib/async-loop/cpp/loop.h>
#include <lib/async/cpp/task.h>
#include <lib/async/dispatcher.h>

#include "garnet/drivers/bluetooth/lib/gatt/gatt.h"

#include "lib/fxl/macros.h"

namespace bthost {

// This class is responsible for bridging remote GATT services to the DDK so
// GATT services can be implimented as drivers (eg HID over GATT as HIDBUS
// device)
//
// THREAD SAFETY: This class is threadsafe.
class GattRemoteServiceDevice final
    : public fxl::RefCountedThreadSafe<GattRemoteServiceDevice> {
 public:
  // Creates the device and makes itself bindable by any DDK driver
  zx_status_t Bind();

  // Explictly removes the device.
  zx_status_t Shutdown();

 private:
  FRIEND_MAKE_REF_COUNTED(GattRemoteServiceDevice);
  FRIEND_REF_COUNTED_THREAD_SAFE(GattRemoteServiceDevice);

  GattRemoteServiceDevice(zx_device_t* parent_device,
                          const std::string& peer_id,
                          async_dispatcher_t* dispatcher,
                          fbl::RefPtr<btlib::gatt::RemoteService> service);

  ~GattRemoteServiceDevice();

  // This structure is static and only contains pointers to static methods in
  // this class. This field is passed by address to the DDK and the DDK does not
  // copy it so it needs to exist as long as this driver is in memory and any
  // child devices may exist.
  static bt_gatt_svc_ops_t proto_ops_;

  // Protocol trampolines.
  static void DdkUnbind(void* ctx) {
    static_cast<GattRemoteServiceDevice*>(ctx)->DdkUnbind();
  }

  static void DdkRelease(void* ctx) {
    static_cast<GattRemoteServiceDevice*>(ctx)->DdkRelease();
  }

  static zx_status_t OpConnect(void* ctx, void* cookie,
                               bt_gatt_connect_cb connect_cb) {
    return static_cast<GattRemoteServiceDevice*>(ctx)->Connect(cookie,
                                                               connect_cb);
  }

  static void OpStop(void* ctx) {
    static_cast<GattRemoteServiceDevice*>(ctx)->Stop();
  }

  static zx_status_t OpReadCharacteristic(
      void* ctx, bt_gatt_id_t id, void* cookie,
      bt_gatt_read_characteristic_cb read_cb) {
    return static_cast<GattRemoteServiceDevice*>(ctx)->ReadCharacteristic(
        id, cookie, read_cb);
  }

  static zx_status_t OpReadLongCharacteristic(
      void* ctx, bt_gatt_id_t id, void* cookie, uint16_t offset,
      size_t max_bytes, bt_gatt_read_characteristic_cb read_cb) {
    return static_cast<GattRemoteServiceDevice*>(ctx)->ReadLongCharacteristic(
        id, cookie, offset, max_bytes, read_cb);
  }

  static zx_status_t OpWriteCharacteristic(void* ctx, bt_gatt_id_t id,
                                           void* cookie, const uint8_t* buf,
                                           size_t len,
                                           bt_gatt_status_cb status_cb) {
    return static_cast<GattRemoteServiceDevice*>(ctx)->WriteCharacteristic(
        id, cookie, buf, len, status_cb);
  }

  static zx_status_t OpEnableNotifications(
      void* ctx, bt_gatt_id_t id, void* cookie, bt_gatt_status_cb status_cb,
      bt_gatt_notification_value_cb value_cb) {
    return static_cast<GattRemoteServiceDevice*>(ctx)->EnableNotifications(
        id, cookie, status_cb, value_cb);
  }

  // DDK device ops.
  void DdkUnbind();
  void DdkRelease();

  // bt-gatt-svc ops.
  zx_status_t Connect(void* cookie, bt_gatt_connect_cb connect_cb);
  void Stop();
  zx_status_t ReadCharacteristic(bt_gatt_id_t id, void* cookie,
                                 bt_gatt_read_characteristic_cb read_cb);

  zx_status_t ReadLongCharacteristic(bt_gatt_id_t id, void* cookie,
                                     uint16_t offset, size_t max_bytes,
                                     bt_gatt_read_characteristic_cb read_cb);
  zx_status_t WriteCharacteristic(bt_gatt_id_t id, void* cookie,
                                  const uint8_t* buff, size_t len,
                                  bt_gatt_status_cb write_cb);
  zx_status_t EnableNotifications(bt_gatt_id_t id, void* cookie,
                                  bt_gatt_status_cb status_cb,
                                  bt_gatt_notification_value_cb value_cb);

  // Guards access to members below.
  std::mutex mtx_;

  // All device protocol messages are dispatched on this loop to not block the
  // gatt or host thread.
  async_dispatcher_t* dispatcher_;

  zx_device_t* parent_device_;  // The BT Host device
  zx_device_t* dev_;  // The device owned by this class (or rather who owns us
                      // from a memory management standpoint).

  const std::string& peer_id_;
  fbl::RefPtr<btlib::gatt::RemoteService> service_ __TA_GUARDED(mtx_);

  std::unordered_map<btlib::gatt::IdType, btlib::gatt::IdType> notify_handlers_
      __TA_GUARDED(mtx_);

  // The base DDK device ops.
  zx_protocol_device_t dev_proto_ = {};

  bool stopped_ __TA_GUARDED(mtx_);
  bool unbound_ __TA_GUARDED(mtx_);
  bool shutdown_ __TA_GUARDED(mtx_);

  FXL_DISALLOW_COPY_AND_ASSIGN(GattRemoteServiceDevice);
};

}  // namespace bthost
#endif  // GARNET_DRIVERS_BLUETOOTH_HOST_GATT_REMOTE_SERVICE_DEVICE_H_
