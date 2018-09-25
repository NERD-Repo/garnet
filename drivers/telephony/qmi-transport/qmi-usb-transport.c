// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <ddk/protocol/qmi.h>
#include <zircon/device/qmi-transport.h>
#include <ddk/binding.h>
#include <ddk/device.h>
#include <lib/sync/completion.h>
#include <ddk/driver.h>
#include <ddk/protocol/usb.h>
#include <ddk/usb/usb.h>
#include <zircon/listnode.h>
#include <zircon/status.h>
#include <zircon/syscalls/port.h>
#include <zircon/hw/usb-cdc.h>
#include "qmi.h"

#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define _ALL_SOURCE
#include <threads.h>
#include <unistd.h>

// qmi usb transport device
typedef struct qmi_ctx {
    // Interrupt handling
    usb_request_t* int_txn_buf;
    sync_completion_t completion;
    thrd_t int_thread;

    // Port to watch for QMI messages on
    zx_handle_t channel_port;
    zx_handle_t channel;

    usb_protocol_t usb;
    zx_device_t* usb_device;
    zx_device_t* zxdev;

    mtx_t mutex;
} qmi_ctx_t;

static zx_status_t get_channel(void* ctx, zx_handle_t* out_channel) {
    zxlogf(INFO, "Getting channel from QMI transport!\n");
    qmi_ctx_t* qmi_ctx = ctx;
    zx_status_t result = ZX_OK;
    mtx_lock(&qmi_ctx->mutex);

    zx_handle_t* in_channel = &qmi_ctx->channel;

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

done:
    mtx_unlock(&qmi_ctx->mutex);
    return result;
}

static zx_status_t qmi_ioctl(void* ctx, uint32_t op,
                             const void* in_buf, size_t in_len,
                             void* out_buf, size_t out_len, size_t* out_actual) {
  printf("qmi: IOCTL CALLED: %d\n", op);
  qmi_ctx_t* qmi_ctx = ctx;
  zx_status_t status = ZX_OK;
  if (op != IOCTL_QMI_GET_CHANNEL) {
    status = ZX_ERR_NOT_SUPPORTED;
    goto done;
  }

  if (out_buf == NULL || out_len < sizeof(zx_handle_t)) {
    status = ZX_ERR_INVALID_ARGS;
    goto done;
  }

  if (status == ZX_OK) {
    get_channel(ctx, (zx_handle_t*)out_buf);
    *out_actual = sizeof(zx_handle_t);
  }

  status = zx_object_wait_async(qmi_ctx->channel, qmi_ctx->channel_port, 42, ZX_CHANNEL_READABLE, ZX_WAIT_ASYNC_REPEATING);

  printf("qmi: OPENED CHANNEL\n");

done:
  return status;
}

static zx_protocol_device_t qmi_ops = {
    .version = DEVICE_OPS_VERSION,
    .ioctl = qmi_ioctl,
};

static void qmi_handle_interrupt(qmi_ctx_t* qmi_ctx, usb_request_t* request) {
    if (request->response.actual < sizeof(usb_cdc_notification_t)) {
        zxlogf(ERROR, "qmi: ignored interrupt (size = %ld)\n", (long)request->response.actual);
        return;
    }

    usb_cdc_notification_t usb_req;
    usb_req_copy_from(&qmi_ctx->usb, request, &usb_req, sizeof(usb_cdc_notification_t), 0);

    zxlogf(INFO, "qmi: Notification Available\n");
    uint8_t buffer[512];
    zx_status_t status;
    switch (usb_req.bNotification) {
      case USB_CDC_NC_RESPONSE_AVAILABLE:
        status = usb_control(&qmi_ctx->usb, USB_DIR_IN | USB_TYPE_CLASS | USB_RECIP_INTERFACE,
            USB_CDC_GET_ENCAPSULATED_RESPONSE, 0, 8, buffer, 512, ZX_TIME_INFINITE, NULL);
        printf("qmi: control response: %s\n", zx_status_get_string(status));
        mtx_lock(&qmi_ctx->mutex);
        status = zx_channel_write(qmi_ctx->channel, 0, buffer, 512, NULL, 0);
        mtx_unlock(&qmi_ctx->mutex);
        if (status < 0) {
          zxlogf(ERROR, "qmi: failed to write message to channel: %s\n", zx_status_get_string(status));
        }
        return;
      case USB_CDC_NC_NETWORK_CONNECTION:
        zxlogf(INFO, "qmi: Network Status: %d\n", usb_req.wValue);
        return;
      default:
        zxlogf(INFO, "qmi: Unknown Notification Type: %d\n", usb_req.bNotification);
    }
}

static void qmi_interrupt_cb(usb_request_t* req, void* cookie) {
    qmi_ctx_t* qmi_ctx = (qmi_ctx_t*)cookie;

    mtx_lock(&qmi_ctx->mutex);
    zxlogf(ERROR, "qmi: Interupt callback called!\n");
    zx_port_packet_t packet = {};
    packet.key = 43;
    zx_port_queue(qmi_ctx->channel_port, &packet);
    mtx_unlock(&qmi_ctx->mutex);
}

static int qmi_int_handler_thread(void* cookie) {
    qmi_ctx_t* ctx = cookie;
    usb_request_t* txn = ctx->int_txn_buf;
    zxlogf(ERROR, "qmi: starting interupt handler thread\n");

    usb_request_queue(&ctx->usb, txn);

    uint8_t buffer[512];
    uint32_t length = sizeof(buffer);
    zx_port_packet_t packet;
    while (true) {
      printf("qmi: up top\n");
      zx_status_t status = zx_port_wait(ctx->channel_port, ZX_TIME_INFINITE, &packet);
      if (status == ZX_ERR_TIMED_OUT) {
        printf("qmi: timed out: %s\n", zx_status_get_string(status));
      } else {
        printf("qmi: packet key: %lu\n", packet.key);
        if (packet.key == 42) {
          printf("qmi: got channel msg: %s\n", zx_status_get_string(status));
          zx_channel_read(ctx->channel, 0, buffer, NULL, sizeof(buffer), 0, &length, NULL);
          printf("qmi: length of message: %d\n", length);
          status = usb_control(&ctx->usb, USB_DIR_OUT | USB_TYPE_CLASS | USB_RECIP_INTERFACE,
              USB_CDC_SEND_ENCAPSULATED_COMMAND, 0, 8, buffer, length, ZX_TIME_INFINITE, NULL);
          if (status < 0) {
            printf("qmi: got an bad status from usb_control: %s\n", zx_status_get_string(status));
            return status;
          }
        } else if (packet.key == 43) {
          usb_request_queue(&ctx->usb, txn);
          if (txn->response.status == ZX_OK) {
            qmi_handle_interrupt(ctx, txn);
          } else if (txn->response.status == ZX_ERR_PEER_CLOSED || txn->response.status == ZX_ERR_IO_NOT_PRESENT) {
            zxlogf(INFO, "qmi: terminating interrupt handling thread\n");
            return txn->response.status;
          }
        }
      }
    }
}

static zx_status_t qmi_bind(void* ctx, zx_device_t* device) {
    qmi_ctx_t* qmi_ctx;
    if ((qmi_ctx = calloc(1, sizeof(qmi_ctx_t))) == NULL) {
        return ZX_ERR_NO_MEMORY;
    }

    zx_status_t status;

    // Set up USB stuff
    usb_protocol_t usb;
    status = device_get_protocol(device, ZX_PROTOCOL_USB, &usb);
    if (status != ZX_OK) {
        zxlogf(ERROR, "qmi: get protocol failed: %s\n", zx_status_get_string(status));
        return status;
    }

    // Initialize context
    qmi_ctx->usb_device = device;
    memcpy(&qmi_ctx->usb, &usb, sizeof(qmi_ctx->usb));

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


    // set up interrupt
    usb_request_t* int_buf;
    status = usb_req_alloc(&usb, &int_buf, intr_max_packet, intr_addr);
    if (status != ZX_OK) {
      goto fail;
    }
    int_buf->complete_cb = qmi_interrupt_cb;
    int_buf->cookie = qmi_ctx;
    qmi_ctx->int_txn_buf = int_buf;

    // create port to watch for interrupts and channel messages
    if (qmi_ctx->channel_port == ZX_HANDLE_INVALID) {
      zx_status_t status = zx_port_create(0, &qmi_ctx->channel_port);
        if (status != ZX_OK) {
            printf(
                "qmi: failed to create a port: "
                "%s\n",
                zx_status_get_string(status));
            goto fail;
        }
    }

    // Kick off the handler thread
    int thread_result = thrd_create_with_name(&qmi_ctx->int_thread, qmi_int_handler_thread,
        qmi_ctx, "qmi_int_handler_thread");
    if (thread_result != thrd_success) {
      zxlogf(ERROR, "qmi: failed to create interrupt handler thread (%d)\n", thread_result);
      goto fail;
    }

    // Add the devices

    device_add_args_t args = {
        .version = DEVICE_ADD_ARGS_VERSION,
        .name = "qmi-usb-transport",
        .ctx = qmi_ctx,
        .ops = &qmi_ops,
        .proto_id = ZX_PROTOCOL_QMI_TRANSPORT,
    };

    if ((status = device_add(device, &args, &qmi_ctx->zxdev)) < 0) {
        goto fail;
    }

    return ZX_OK;

fail:
    zxlogf(ERROR, "qmi: bind failed: %s\n", zx_status_get_string(status));
    free(qmi_ctx);
    return status;
}

static zx_driver_ops_t qmi_driver_ops = {
    .version = DRIVER_OPS_VERSION,
    .bind = qmi_bind,
};

// clang-format off
ZIRCON_DRIVER_BEGIN(qmi_usb, qmi_driver_ops, "zircon", "0.1", 3)
    BI_ABORT_IF(NE, BIND_PROTOCOL, ZX_PROTOCOL_USB),
    BI_ABORT_IF(NE, BIND_USB_VID, SIERRA_VID),
    BI_MATCH_IF(EQ, BIND_USB_PID, EM7565_PID),
ZIRCON_DRIVER_END(qmi_usb)
