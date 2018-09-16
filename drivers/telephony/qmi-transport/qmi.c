// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <ddk/protocol/qmi.h>
#include <ddk/binding.h>
#include <ddk/device.h>
#include <ddk/driver.h>
//#include <ddk/protocol/bt-hci.h>
#include <ddk/protocol/usb.h>
#include <ddk/usb/usb.h>
//#include <zircon/device/bt-hci.h>
#include <zircon/listnode.h>
#include <zircon/status.h>
#include <zircon/syscalls/port.h>
#include <zircon/hw/usb-cdc.h>
#include "qmi.h"

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <threads.h>
#include <unistd.h>

typedef struct {
    zx_device_t* zxdev;
    zx_device_t* usb_zxdev;
    usb_protocol_t usb;

    usb_request_t* req;
    //mtx_t read_mutex;
    //mtx_t write_mutex;

//    uint8_t packet_size;

    list_node_t free_event_reqs;

    zx_handle_t channel;
} qmi_t;

static void intr_cb(usb_request_t* req, void* cookie) {
    qmi_t* qmi = (qmi_t*)cookie;

    if (req->response.status == ZX_OK) {
      if (req->response.actual < sizeof(usb_cdc_notification_t)) {
        zxlogf(ERROR, "qmi: ignored interrupt (size = %ld)\n", (long)req->response.actual);
        return;
      }

      usb_cdc_notification_t usb_req;
      usb_req_copy_from(&qmi->usb, req, &usb_req, sizeof(usb_cdc_notification_t), 0);
      zxlogf(ERROR, "qmi: Notification type: %s\n", zx_status_get_string(req->response.status));

      //void* buffer;
      //zx_status_t status = usb_req_mmap(&qmi->usb, req, &buffer);
      //if (status != ZX_OK) {
      //    zxlogf(ERROR, "qmi: usb_req_mmap failed: %s\n", zx_status_get_string(status));
      //    return;
      //}
      //size_t length = req->response.actual;
      //zxlogf(ERROR, "qmi: Recieved response of size: %d\n", (int)length);
    }
    zxlogf(ERROR, "qmi: response not ok: %s\n", zx_status_get_string(req->response.status));
}

static zx_status_t qmi_open_channel_helper(qmi_t* hci, zx_handle_t* in_channel, zx_handle_t* out_channel) {
  zx_status_t result = ZX_OK;
  //mtx_lock(&hci->mutex);

  if (*in_channel != ZX_HANDLE_INVALID) {
    printf("qmi: already bound, failing\n");
    result = ZX_ERR_ALREADY_BOUND;
    goto done;
  }

  zx_status_t status = zx_channel_create(0, in_channel, out_channel);
  if (status < 0) {
    printf("qmi: Failed to create channel: %s\n", zx_status_get_string(status));
    result = ZX_ERR_INTERNAL;
    goto done;
  }

  // Kick off the hci_read_thread if it's not already running.
  //if (!hci->read_thread_running) {
  //  hci_build_read_wait_items_locked(hci);
  //  thrd_t read_thread;
  //  thrd_create_with_name(&read_thread, hci_read_thread, hci, "bt_usb_read_thread");
  //  hci->read_thread_running = true;
  //  thrd_detach(read_thread);
  //} else {
  //  // Poke the changed event to get the new channel.
  //  zx_object_signal(hci->channels_changed_evt, 0, ZX_EVENT_SIGNALED);
  //}


done:
//    mtx_unlock(&hci->mutex);
    return result;
}
static zx_status_t open_qmi_channel(void* ctx, zx_handle_t* out_channel) {
    qmi_t* qmi = ctx;
    return qmi_open_channel_helper(qmi, &qmi->channel, out_channel);
}

static qmi_protocol_ops_t qmi_protocol_ops = {
    .open_channel = open_qmi_channel,
};

static zx_status_t qmi_get_protocol(void* ctx, uint32_t proto_id, void* protocol) {
//    qmi_t* qmi = ctx;
    //if (proto_id != ZX_PROTOCOL_BT_HCI) {
    //    // Pass this on for drivers to load firmware / initialize
    //    return device_get_protocol(hci->usb_zxdev, proto_id, protocol);
    //}

    qmi_protocol_t* qmi_proto = protocol;

    qmi_proto->ops = &qmi_protocol_ops;
    qmi_proto->ctx = ctx;
    return ZX_OK;
};

static zx_protocol_device_t qmi_device_proto = {
    .version = DEVICE_OPS_VERSION,
    .get_protocol = qmi_get_protocol,
    //.unbind = hci_unbind,
    //.release = hci_release,
};

static zx_status_t qmi_bind(void* ctx, zx_device_t* device) {
    usb_protocol_t usb;

    zx_status_t status = device_get_protocol(device, ZX_PROTOCOL_USB, &usb);
    if (status != ZX_OK) {
        zxlogf(ERROR, "qmi: get protocol failed: %s\n", zx_status_get_string(status));
        return status;
    }

    // find our endpoints
    usb_desc_iter_t iter;
    zx_status_t result = usb_desc_iter_init(&usb, &iter);
    if (result < 0) {
        zxlogf(ERROR, "qmi: usb iterator failed: %s\n", zx_status_get_string(status));
        return result;
    }

    // QMI needs to bind to interface 8. Ignore the others for now.

    usb_interface_descriptor_t* intf = usb_desc_iter_next_interface(&iter, true);
    zxlogf(ERROR, "qmi: Attempting to bind to Interface Number: %d\n", intf->bInterfaceNumber);

    if (!intf || intf->bInterfaceNumber != QMI_INTERFACE_NUM) {
        zxlogf(ERROR, "qmi: QMI is only available on interface %d\n", QMI_INTERFACE_NUM);
        usb_desc_iter_release(&iter);
        return ZX_ERR_NOT_SUPPORTED;
    }

    if (intf->bNumEndpoints != 3) {
        zxlogf(ERROR, "qmi: interface does not have the required 3 endpoints: %s\n", zx_status_get_string(status));
        usb_desc_iter_release(&iter);
        return ZX_ERR_NOT_SUPPORTED;
    }

    uint8_t bulk_in_addr = 0;
    uint8_t bulk_out_addr = 0;
    uint8_t intr_addr = 0;
    uint16_t intr_max_packet = 0;

   // usb_endpoint_descriptor_t* endp = usb_desc_iter_next_endpoint(&iter);
    usb_descriptor_header_t* desc = usb_desc_iter_next(&iter);
    while (desc) {
    zxlogf(INFO, "qmi: Descriptor Type %d\n", desc->bDescriptorType);
      if (desc->bDescriptorType == USB_DT_ENDPOINT) {
        usb_endpoint_descriptor_t* endp = (void*)desc;
        if (usb_ep_direction(endp) == USB_ENDPOINT_OUT) {
            if (usb_ep_type(endp) == USB_ENDPOINT_BULK) {
                bulk_out_addr = endp->bEndpointAddress;
            }
        } else {
            if (usb_ep_type(endp) == USB_ENDPOINT_BULK) {
                bulk_in_addr = endp->bEndpointAddress;
            } else if (usb_ep_type(endp) == USB_ENDPOINT_INTERRUPT) {
                intr_addr = endp->bEndpointAddress;
                intr_max_packet = usb_ep_max_packet(endp);
            }
        }
      }
      desc = usb_desc_iter_next(&iter);
    }
    usb_desc_iter_release(&iter);

    if (!bulk_in_addr || !bulk_out_addr || !intr_addr) {
        zxlogf(ERROR, "qmi: bind could not find endpoints\n");
        return ZX_ERR_NOT_SUPPORTED;
    }
    zxlogf(INFO, "qmi: found all the endpoints\n");
    zxlogf(INFO, "qmi: Max Packet Size: %d\n", intr_max_packet);

    ////
    // WDM style control management
    qmi_t* qmi = calloc(1, sizeof(qmi_t));
    if (!qmi) {
      printf("qmi: Not enough memory for qmi_t\n");
      return ZX_ERR_NO_MEMORY;
    }



    //qmi->packet_size = intr_max_packet;


    zxlogf(INFO, "qmi: Max Packet Size: %d\n", intr_max_packet);
    status = usb_req_alloc(&usb, &qmi->req, intr_max_packet, intr_addr);
    if (status != ZX_OK) {
      usb_desc_iter_release(&iter);
      free(qmi);
      goto fail;
    }
    qmi->req->complete_cb = intr_cb;
    qmi->req->cookie = qmi;


    device_add_args_t args = {
        .version = DEVICE_ADD_ARGS_VERSION,
        .name = "qmi_transport",
        .ctx = qmi,
        .ops = &qmi_device_proto,
        .proto_id = ZX_PROTOCOL_QMI_TRANSPORT,
        //.props = props,
        //.prop_count = countof(props),
    };

    status = device_add(device, &args, &qmi->zxdev);
    if (status == ZX_OK) {
        return ZX_OK;
    }

fail:
    zxlogf(ERROR, "qmi: bind failed: %s\n", zx_status_get_string(status));
//    hci_release(hci);
    return status;
}

static zx_driver_ops_t qmi_driver_ops = {
    .version = DRIVER_OPS_VERSION,
    .bind = qmi_bind,
    //.release = TODO
};

// clang-format off
ZIRCON_DRIVER_BEGIN(qmi_usb, qmi_driver_ops, "zircon", "0.1", 3)
    BI_ABORT_IF(NE, BIND_PROTOCOL, ZX_PROTOCOL_USB),
    BI_ABORT_IF(NE, BIND_USB_VID, SIERRA_VID),
    BI_MATCH_IF(EQ, BIND_USB_PID, EM7565_PID),
ZIRCON_DRIVER_END(qmi_usb)

