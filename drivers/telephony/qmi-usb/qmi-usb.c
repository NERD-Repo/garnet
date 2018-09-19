// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "qmi-usb.h"

#include <ddk/binding.h>
#include <ddk/device.h>
#include <ddk/driver.h>
//#include <ddk/protocol/bt-hci.h>
#include <ddk/protocol/usb.h>
#include <ddk/usb/usb.h>
#include <zircon/listnode.h>
#include <zircon/status.h>
#include <zircon/syscalls/port.h>

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <threads.h>
#include <unistd.h>

// TODO rename qmi-usb to something better

static zx_status_t qmi_bind(void* ctx, zx_device_t* device) {
  usb_protocol_t usb;

    zx_status_t status = device_get_protocol(device, ZX_PROTOCOL_USB, &usb);
    if (status != ZX_OK) {
        zxlogf(ERROR, "%s: device_get_protocol failed %d\n", __FUNCTION__, status);
        goto fail;
    }

    device_add_args_t args = {
      .version = DEVICE_ADD_ARGS_VERSION,
      .name = "qmi-transport",
      .ctx = cdc,
      .ops = &usb_cdc_proto,
      //.proto_id = ZX_PROTOCOL_ETHERNET_IMPL,
      //.proto_ops = &ethmac_ops,
    };

    status = device_add(parent, &args, &cdc->zxdev);
    if (status != ZX_OK) {
        zxlogf(ERROR, "%s: add_device failed %d\n", __FUNCTION__, status);
        goto fail;
    }

fail:
    printf("qmi-usb: bind failed: %s\n", zx_status_get_string(status));
    hci_release(hci);
    return status;
};

static zx_driver_ops_t qmi_driver_ops = {
    .version = DRIVER_OPS_VERSION,
    .bind = qmi_bind,
};

// clang-format off
ZIRCON_DRIVER_BEGIN(qmi_usb, qmi_driver_ops, "zircon", "0.1", 2) // UPDATE THAT NUMBER AT THE END!
    BI_ABORT_IF(NE, BIND_PROTOCOL, ZX_PROTOCOL_USB),
//    BI_MATCH_IF(EQ, BIND_USB_PROTOCOL, USB_PROTOCOL_QMI),
ZIRCON_DRIVER_END(qmi_usb)
