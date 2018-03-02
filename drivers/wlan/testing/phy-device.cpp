// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "phy-device.h"

#include "driver.h"
#include "iface-device.h"
#include "garnet/lib/wlan/fidl/iface.fidl.h"
#include "garnet/lib/wlan/fidl/phy.fidl.h"
#include "iface-device.h"

#include <ddk/debug.h>
#include <wlan/protocol/ioctl.h>
#include <wlan/protocol/phy.h>

#include <stdio.h>
#include <algorithm>

namespace wlan {
namespace testing {

#define DEV(c) static_cast<PhyDevice*>(c)
static zx_protocol_device_t wlanphy_test_device_ops = {
    .version = DEVICE_OPS_VERSION,
    .get_protocol = nullptr,
    .open = nullptr,
    .open_at = nullptr,
    .close = nullptr,
    .unbind = [](void* ctx) { DEV(ctx)->Unbind(); },
    .release = [](void* ctx) { DEV(ctx)->Release(); },
    .read = nullptr,
    .write = nullptr,
    .get_size = nullptr,
    .ioctl = [](void* ctx, uint32_t op, const void* in_buf, size_t in_len, void* out_buf,
                size_t out_len, size_t* out_actual) -> zx_status_t {
        return DEV(ctx)->Ioctl(op, in_buf, in_len, out_buf, out_len, out_actual);
    },
    .suspend = nullptr,
    .resume = nullptr,
    .rxrpc = nullptr,
};
#undef DEV

static wlanphy_protocol_ops_t wlanphy_test_ops = {
    .reserved = 0,
};

PhyDevice::PhyDevice(zx_device_t* device) : parent_(device) {}

zx_status_t PhyDevice::Bind() {
    zxlogf(INFO, "wlan::testing::phy::PhyDevice::Bind()\n");

    dispatcher_ = std::make_unique<wlan::dispatcher::Dispatcher<wlan::Phy>>(wlanphy_async_t());

    device_add_args_t args = {};
    args.version = DEVICE_ADD_ARGS_VERSION;
    args.name = "wlanphy-test";
    args.ctx = this;
    args.ops = &wlanphy_test_device_ops;
    args.proto_id = ZX_PROTOCOL_WLANPHY;
    args.proto_ops = &wlanphy_test_ops;

    zx_status_t status = device_add(parent_, &args, &zxdev_);
    if (status != ZX_OK) {
        printf("wlanphy-test: could not add test device: %d\n", status);
        return status;
    }

    return status;
}

void PhyDevice::Unbind() {
    zxlogf(INFO, "wlan::testing::PhyDevice::Unbind()\n");
    std::lock_guard<std::mutex> guard(lock_);
    dispatcher_.reset();
    device_remove(zxdev_);
}

void PhyDevice::Release() {
    zxlogf(INFO, "wlan::testing::PhyDevice::Release()\n");
    delete this;
}

zx_status_t PhyDevice::Ioctl(uint32_t op, const void* in_buf, size_t in_len, void* out_buf,
                             size_t out_len, size_t* out_actual) {
    zxlogf(INFO, "wlan::testing::phy::PhyDevice::Ioctl()\n");
    switch (op) {
    case IOCTL_WLANPHY_CONNECT:
        zxlogf(INFO, "wlanphy ioctl: connect\n");
        return Connect(in_buf, in_len);
    case IOCTL_WLANPHY_QUERY:
        zxlogf(INFO, "wlanphy ioctl: query len=%zu\n", out_len);
        return Query(static_cast<uint8_t*>(out_buf), out_len, out_actual);
    case IOCTL_WLANPHY_CREATE_IFACE:
        zxlogf(INFO, "wlanphy ioctl: create if inlen=%zu outlen=%zu\n", in_len, out_len);
        return CreateIface(in_buf, in_len, out_buf, out_len, out_actual);
    case IOCTL_WLANPHY_DESTROY_IFACE:
        zxlogf(INFO, "wlanphy ioctl: destroy if inlen=%zu\n", in_len);
        *out_actual = 0;
        return DestroyIface(in_buf, in_len);
    default:
        zxlogf(ERROR, "wlanphy ioctl: unknown (%u)\n", op);
        return ZX_ERR_NOT_SUPPORTED;
    }
}

namespace {
wlan::WlanPhyInfo get_info() {
    WlanPhyInfo info;
    info.supported_phys.resize(0);
    info.driver_features.resize(0);
    info.mac_roles.resize(0);
    info.caps.resize(0);
    info.bands.resize(0);

    info.supported_phys->push_back(wlan::SupportedPhy::DSSS);
    info.supported_phys->push_back(wlan::SupportedPhy::CCK);
    info.supported_phys->push_back(wlan::SupportedPhy::OFDM);
    info.supported_phys->push_back(wlan::SupportedPhy::HT);

    info.mac_roles->push_back(wlan::MacRole::CLIENT);
    info.mac_roles->push_back(wlan::MacRole::AP);

    info.caps->push_back(wlan::Capability::SHORT_PREAMBLE);
    info.caps->push_back(wlan::Capability::SHORT_SLOT_TIME);

    BandInfo band24;
    band24.description = "2.4 GHz";
    band24.ht_caps.ht_capability_info = 0x01fe;
    auto& band24mcs = band24.ht_caps.supported_mcs_set;
    std::fill(band24mcs.begin(), band24mcs.end(), 0);
    band24mcs[0] = 0xff;
    band24mcs[3] = 0x80;
    band24mcs[12] = 0x10;
    band24.basic_rates.reset(std::vector<uint8_t>({2, 4, 11, 22, 12, 18, 24, 36, 48, 72, 96, 108}));
    band24.supported_channels.base_freq = 2417;
    band24.supported_channels.channels.reset(
            std::vector<uint8_t>({1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14}));

    info.bands->push_back(std::move(band24));

    BandInfo band5;
    band5.description = "5 GHz";
    band5.ht_caps.ht_capability_info = 0x01fe;
    auto& band5mcs = band5.ht_caps.supported_mcs_set;
    std::fill(band5mcs.begin(), band5mcs.end(), 0);
    band5mcs[0] = 0xff;
    band5mcs[1] = 0xff;
    band5mcs[3] = 0x80;
    band5mcs[12] = 0x10;
    band5.basic_rates.reset(std::vector<uint8_t>({12, 18, 24, 36, 48, 72, 96, 108}));
    band5.supported_channels.base_freq = 5000;
    band5.supported_channels.channels.reset(
            std::vector<uint8_t>(
                {36, 38,  40,  42,  44,  46,  48,  50,  52,  54,  56,  58,
                60,  62,  64,  100, 102, 104, 106, 108, 110, 112, 114, 116,
                118, 120, 122, 124, 126, 128, 130, 132, 134, 136, 138, 140,
                149, 151, 153, 155, 157, 159, 161, 165, 184, 188, 192, 196}));

    info.bands->push_back(std::move(band5));
    return info;
}
}  // namespace

void PhyDevice::Query(QueryCallback callback) {
    zxlogf(INFO, "wlan::testing::phy::PhyDevice::Query()\n");
    QueryResponse resp;
    resp.info = get_info();
    callback(std::move(resp));
}

void PhyDevice::CreateIface(CreateIfaceRequest req,
                            CreateIfaceCallback callback) {
    zxlogf(INFO, "CreateRequest: role=%u\n", req.role);
    std::lock_guard<std::mutex> guard(lock_);
    CreateIfaceResponse resp;

    // We leverage wrapping of unsigned ints to cycle back through ids to find an unused one.
    bool found_unused = false;
    uint16_t id = next_id_;
    while (!found_unused) {
        if (ifaces_.count(id) > 0) {
            id++;
            // If we wrap all the way around, something is very wrong.
            if (next_id_ == id) { break; }
        } else {
            found_unused = true;
        }
    }
    ZX_DEBUG_ASSERT(found_unused);
    if (!found_unused) {
        resp.status_ = ZX_ERR_NO_RESOURCES;
        callback(std::move(resp));
        return;
    }

    // Create the interface device and bind it.
    auto macdev = std::make_unique<IfaceDevice>(zxdev_);
    zx_status_t status = macdev->Bind();
    if (status != ZX_OK) {
        zxlogf(ERROR, "could not bind child wlanmac device: %d\n", status);
        resp.status_ = status;
        callback(std::move(resp));
        return;
    }

    // Memory management follows the device lifecycle at this point. The only way an interface
    // can be removed is through this phy device, either through a "destroy interface" ioctl or
    // by the phy going away, so it should be safe to store the raw pointer.
    ifaces_[id] = macdev.release();

    // Since we successfully used the id, increment the next id counter.
    next_id_ = id + 1;

    resp.status_ = ZX_OK;
    callback(std::move(resp));
}

void PhyDevice::DestroyIface(DestroyIfaceRequest req,
                             DestroyIfaceCallback callback) {
    zxlogf(INFO, "DestroyRequest: id=%u\n", req.id);

    DestroyIfaceResponse resp;

    std::lock_guard<std::mutex> guard(lock_);
    auto intf = ifaces_.find(req.id);
    if (intf == ifaces_.end()) {
        resp.status_ = ZX_ERR_NOT_FOUND;
        callback(std::move(resp));
        return;
    }

    device_remove(intf->second->zxdev());
    // Remove the device from our map. We do NOT free the memory, since the devhost owns it and will
    // call release when it's safe to free the memory.
    ifaces_.erase(req.id);

    resp.status_ = ZX_OK;
    callback(std::move(resp));
}

zx_status_t PhyDevice::Connect(const void* buf, size_t len) {
    if (buf == nullptr || len < sizeof(zx_handle_t)) {
        return ZX_ERR_BUFFER_TOO_SMALL;
    }

    zx_handle_t hnd = *reinterpret_cast<const zx_handle_t*>(buf);
    zx::channel chan(hnd);

    zx_status_t status = dispatcher_->AddBinding(std::move(chan), this);
    if (status != ZX_OK) { return status; }
    return ZX_OK;
}

zx_status_t PhyDevice::Query(uint8_t* buf, size_t len, size_t* actual) {
    zxlogf(INFO, "wlan::testing::PhyDevice::Query()\n");
    return ZX_ERR_NOT_SUPPORTED;
    //std::lock_guard<std::mutex> guard(lock_);
    //if (dead_) {
    //    return ZX_ERR_PEER_CLOSED;
    //}

    //auto info = get_info();
    //if (len < info->GetSerializedSize()) { return ZX_ERR_BUFFER_TOO_SMALL; }
    //if (!info->Serialize(buf, info->GetSerializedSize(), actual)) { return ZX_ERR_IO; }
    //return ZX_OK;
}

zx_status_t PhyDevice::CreateIface(const void* in_buf, size_t in_len, void* out_buf, size_t out_len,
                                   size_t* out_actual) {
    auto req = wlan::phy::CreateIfaceRequest::New();
    if (!req->Deserialize(const_cast<void*>(in_buf), in_len)) {
        return ZX_ERR_IO;
    }

    zxlogf(INFO, "CreateRequest: role=%u\n", req->role);
    std::lock_guard<std::mutex> guard(lock_);
    if (dead_) { return ZX_ERR_PEER_CLOSED; }

    // We leverage wrapping of unsigned ints to cycle back through ids to find an unused one.
    bool found_unused = false;
    uint16_t id = next_id_;
    while (!found_unused) {
        if (ifaces_.count(id) > 0) {
            id++;
            // If we wrap all the way around, something is very wrong.
            if (next_id_ == id) { break; }
        } else {
            found_unused = true;
        }
    }
    ZX_DEBUG_ASSERT(found_unused);
    if (!found_unused) { return ZX_ERR_NO_RESOURCES; }

    // Build the response now, so if the return buffer is too small we find out before we create the
    // device.
    auto info = wlan::iface::WlanIfaceInfo::New();
    info->id = id;
    if (out_len < info->GetSerializedSize()) { return ZX_ERR_BUFFER_TOO_SMALL; }
    if (!info->Serialize(out_buf, info->GetSerializedSize(), out_actual)) { return ZX_ERR_IO; }


    // Create the interface device and bind it.
    auto macdev = std::make_unique<IfaceDevice>(zxdev_);
    zx_status_t status = macdev->Bind();
    if (status != ZX_OK) {
        // Set the actual length of the output to zero and clear the first few bytes to make sure
        // the serialized response isn't incorrectly interpreted.
        *out_actual = 0;
        memset(out_buf, 0, std::min<size_t>(out_len, 64));
        zxlogf(ERROR, "could not bind child wlanmac device: %d\n", status);
        return status;
    }

    // Memory management follows the device lifecycle at this point. The only way an interface
    // can be removed is through this phy device, either through a "destroy interface" ioctl or
    // by the phy going away, so it should be safe to store the raw pointer.
    ifaces_[id] = macdev.release();

    // Since we successfully used the id, increment the next id counter.
    next_id_ = id + 1;

    return ZX_OK;
}


zx_status_t PhyDevice::DestroyIface(const void* in_buf, size_t in_len) {
    auto req = wlan::phy::DestroyIfaceRequest::New();
    if (!req->Deserialize(const_cast<void*>(in_buf), in_len)) { return ZX_ERR_IO; }
    zxlogf(INFO, "DestroyRequest: id=%u\n", req->id);

    std::lock_guard<std::mutex> guard(lock_);
    if (dead_) { return ZX_ERR_PEER_CLOSED; }

    auto intf = ifaces_.find(req->id);
    if (intf == ifaces_.end()) { return ZX_ERR_NOT_FOUND; }

    device_remove(intf->second->zxdev());
    // Remove the device from our map. We do NOT free the memory, since the devhost owns it and will
    // call release when it's safe to free the memory.
    ifaces_.erase(req->id);

    return ZX_OK;
}

}  // namespace testing
}  // namespace wlan
