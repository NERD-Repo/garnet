// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#define _ALL_SOURCE
#include <threads.h>
#include <zircon/status.h>
#include <ddk/protocol/qmi.h>
#include <ddk/binding.h>
#include <zircon/syscalls/port.h>
#include "qmi.h"

#include <ddk/debug.h>
#include <ddk/device.h>
#include <ddk/driver.h>
#include <ddk/protocol/ethernet.h>
#include <ddk/protocol/usb.h>
#include <ddk/usb/usb.h>
#include <zircon/hw/usb-cdc.h>
#include <lib/sync/completion.h>

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CDC_SUPPORTED_VERSION 0x0110 /* 1.10 */

// The maximum amount of memory we are willing to allocate to transaction buffers
#define MAX_TX_BUF_SZ 32768
#define MAX_RX_BUF_SZ 32768

#define ETHMAC_MAX_TRANSMIT_DELAY 100
#define ETHMAC_MAX_RECV_DELAY 100
#define ETHMAC_TRANSMIT_DELAY 10
#define ETHMAC_RECV_DELAY 10
#define ETHMAC_INITIAL_TRANSMIT_DELAY 0
#define ETHMAC_INITIAL_RECV_DELAY 0

const char* module_name = "qmi";

typedef struct {
    uint8_t addr;
    uint16_t max_packet_size;
} ecm_endpoint_t;

typedef struct {
    zx_device_t* zxdev;
    zx_device_t* usb_device;
    usb_protocol_t usb;

    mtx_t ethmac_mutex;
    ethmac_ifc_t* ethmac_ifc;
    void* ethmac_cookie;

    // Device attributes
    uint8_t mac_addr[ETH_MAC_SIZE];
    uint16_t mtu;

    // QMI channel
    zx_handle_t qmi_channel;

    // Connection attributes
    bool online;
    uint32_t ds_bps;
    uint32_t us_bps;

    // Interrupt handling
    ecm_endpoint_t int_endpoint;
    usb_request_t* int_txn_buf;
    sync_completion_t completion;
    thrd_t int_thread;

    // Send context
    mtx_t tx_mutex;
    ecm_endpoint_t tx_endpoint;
    list_node_t tx_txn_bufs;        // list of usb_request_t
    list_node_t tx_pending_infos;   // list of ethmac_netbuf_t
    bool unbound;                   // set to true when device is going away. Guarded by tx_mutex
    uint64_t tx_endpoint_delay;     // wait time between 2 transmit requests

    // Receive context
    ecm_endpoint_t rx_endpoint;
    uint64_t rx_endpoint_delay;    // wait time between 2 recv requests
} ecm_ctx_t;


//typedef struct {
//
//
//} qmi_ctx_t;




static void ecm_unbind(void* cookie) {
    zxlogf(INFO, "%s: unbinding\n", module_name);
    ecm_ctx_t* ctx = cookie;

    mtx_lock(&ctx->tx_mutex);
    ctx->unbound = true;
    if (ctx->ethmac_ifc) {
        ethmac_netbuf_t* netbuf;
        while ((netbuf = list_remove_head_type(&ctx->tx_pending_infos, ethmac_netbuf_t, node)) !=
               NULL) {
            ctx->ethmac_ifc->complete_tx(ctx->ethmac_cookie, netbuf, ZX_ERR_PEER_CLOSED);
        }
    }
    mtx_unlock(&ctx->tx_mutex);

    device_remove(ctx->zxdev);
}

static void ecm_free(ecm_ctx_t* ctx) {
    zxlogf(INFO, "%s: deallocating memory\n", module_name);
    if (ctx->int_thread) {
        thrd_join(ctx->int_thread, NULL);
    }
    usb_request_t* txn;
    while ((txn = list_remove_head_type(&ctx->tx_txn_bufs, usb_request_t, node)) != NULL) {
        usb_req_release(&ctx->usb, txn);
    }
    if (ctx->int_txn_buf) {
        usb_req_release(&ctx->usb, ctx->int_txn_buf);
    }
    mtx_destroy(&ctx->ethmac_mutex);
    mtx_destroy(&ctx->tx_mutex);
    free(ctx);
}

static void ecm_release(void* ctx) {
    ecm_ctx_t* eth = ctx;
    ecm_free(eth);
}

static zx_protocol_device_t ecm_device_proto = {
    .version = DEVICE_OPS_VERSION,
    .unbind = ecm_unbind,
    .release = ecm_release,
};

static zx_status_t open_channel(void* ctx, zx_handle_t* out_channel) {
    zxlogf(INFO, "%s: Opening channel!!!\n", module_name);
    ecm_ctx_t* qmi = ctx;

    zx_handle_t* in_channel = &qmi->qmi_channel;

    zx_status_t result = ZX_OK;
    //mtx_lock(&hci->mutex);

    if (*in_channel != ZX_HANDLE_INVALID) {
        printf("qmi: already bound, failing\n");
        result = ZX_ERR_ALREADY_BOUND;
        goto done;
    }

    zx_status_t status = zx_channel_create(0, in_channel, out_channel);
    if (status < 0) {
        printf("usb: Failed to create channel: %s\n",
               zx_status_get_string(status));
        result = ZX_ERR_INTERNAL;
        goto done;
    }

    // Kick off the hci_read_thread if it's not already running.
    //if (!hci->read_thread_running) {
    //    hci_build_read_wait_items_locked(hci);
    //    thrd_t read_thread;
    //    thrd_create_with_name(&read_thread, hci_read_thread, hci, "bt_usb_read_thread");
    //    hci->read_thread_running = true;
    //    thrd_detach(read_thread);
    //} else {
    //    // Poke the changed event to get the new channel.
    //    zx_object_signal(hci->channels_changed_evt, 0, ZX_EVENT_SIGNALED);
    //}
done:
//    mtx_unlock(&hci->mutex);
    return result;

    return result;
}



static qmi_protocol_ops_t qmi_protocol_ops = {
    .open_channel = open_channel,
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


//static void ecm_update_online_status(ecm_ctx_t* ctx, bool is_online) {
//    mtx_lock(&ctx->ethmac_mutex);
//    if ((is_online && ctx->online) || (!is_online && !ctx->online)) {
//        goto done;
//    }
//
//    if (is_online) {
//        zxlogf(INFO, "%s: connected to network\n", module_name);
//        ctx->online = true;
//        if (ctx->ethmac_ifc) {
//            ctx->ethmac_ifc->status(ctx->ethmac_cookie, ETH_STATUS_ONLINE);
//        } else {
//            zxlogf(ERROR, "%s: not connected to ethermac interface\n", module_name);
//        }
//    } else {
//        zxlogf(INFO, "%s: no connection to network\n", module_name);
//        ctx->online = false;
//        if (ctx->ethmac_ifc) {
//            ctx->ethmac_ifc->status(ctx->ethmac_cookie, 0);
//        }
//    }
//
//done:
//    mtx_unlock(&ctx->ethmac_mutex);
//}

static zx_status_t ethmac_query(void* ctx, uint32_t options, ethmac_info_t* info) {
    ecm_ctx_t* eth = ctx;

    zxlogf(INFO, "%s: %s called\n", module_name, __FUNCTION__);

    // No options are supported
    if (options) {
        zxlogf(ERROR, "%s: unexpected options (0x%"PRIx32") to ethmac_query\n", module_name, options);
        return ZX_ERR_INVALID_ARGS;
    }

    memset(info, 0, sizeof(*info));
    info->mtu = eth->mtu;
    memcpy(info->mac, eth->mac_addr, sizeof(eth->mac_addr));

    return ZX_OK;
}

static void ethmac_stop(void* cookie) {
    zxlogf(INFO, "%s: %s called\n", module_name, __FUNCTION__);
    ecm_ctx_t* ctx = cookie;
    mtx_lock(&ctx->ethmac_mutex);
    ctx->ethmac_ifc = NULL;
    mtx_unlock(&ctx->ethmac_mutex);
}

static zx_status_t ethmac_start(void* ctx_cookie, ethmac_ifc_t* ifc, void* ethmac_cookie) {
    zxlogf(INFO, "%s: %s called\n", module_name, __FUNCTION__);
    ecm_ctx_t* ctx = ctx_cookie;
    zx_status_t status = ZX_OK;

    mtx_lock(&ctx->ethmac_mutex);
    if (ctx->ethmac_ifc) {
        status = ZX_ERR_ALREADY_BOUND;
    } else {
        ctx->ethmac_ifc = ifc;
        ctx->ethmac_cookie = ethmac_cookie;
        ctx->ethmac_ifc->status(ethmac_cookie, ctx->online ? ETH_STATUS_ONLINE : 0);
    }
    mtx_unlock(&ctx->ethmac_mutex);

    return status;
}

static zx_status_t queue_request(ecm_ctx_t* ctx, uint8_t* data, size_t length, usb_request_t* req) {
    zxlogf(INFO, "%s: %s called\n", module_name, __FUNCTION__);
    req->header.length = length;
    ssize_t bytes_copied = usb_req_copy_to(&ctx->usb, req, data, length, 0);
    if (bytes_copied < 0) {
        zxlogf(ERROR, "%s: failed to copy data into send txn (error %zd)\n", module_name, bytes_copied);
        return ZX_ERR_IO;
    }
    usb_request_queue(&ctx->usb, req);
    return ZX_OK;
}

static zx_status_t send_locked(ecm_ctx_t* ctx, ethmac_netbuf_t* netbuf) {
    uint8_t* byte_data = netbuf->data;
    size_t length = netbuf->len;

    // Make sure that we can get all of the tx buffers we need to use
    usb_request_t* tx_req = list_remove_head_type(&ctx->tx_txn_bufs, usb_request_t, node);
    if (tx_req == NULL) {
        return ZX_ERR_SHOULD_WAIT;
    }

    zx_nanosleep(zx_deadline_after(ZX_USEC(ctx->tx_endpoint_delay)));
    zx_status_t status;
    if ((status = queue_request(ctx, byte_data, length, tx_req)) != ZX_OK) {
        list_add_tail(&ctx->tx_txn_bufs, &tx_req->node);
        return status;
    }

    return ZX_OK;
}

static void usb_write_complete(usb_request_t* request, void* cookie) {
    ecm_ctx_t* ctx = cookie;

    if (request->response.status != ZX_OK) {
        zxlogf(INFO, "%s: usb_read_complete called with status %d\n",
                module_name, (int)request->response.status);
    }

    if (request->response.status == ZX_ERR_IO_NOT_PRESENT) {
        usb_req_release(&ctx->usb, request);
        return;
    }

    mtx_lock(&ctx->tx_mutex);

    // Return transmission buffer to pool
    list_add_tail(&ctx->tx_txn_bufs, &request->node);

    if (request->response.status == ZX_ERR_IO_REFUSED) {
        zxlogf(INFO, "%s: resetting transmit endpoint\n", module_name);
        usb_reset_endpoint(&ctx->usb, ctx->tx_endpoint.addr);
    }

    if (request->response.status == ZX_ERR_IO_INVALID) {
        zxlogf(INFO, "%s: slowing down the requests by %d usec."
                     "Resetting the transmit endpoint\n",
               module_name, ETHMAC_TRANSMIT_DELAY);
        if (ctx->tx_endpoint_delay < ETHMAC_MAX_TRANSMIT_DELAY) {
            ctx->tx_endpoint_delay += ETHMAC_TRANSMIT_DELAY;
        }
        usb_reset_endpoint(&ctx->usb, ctx->tx_endpoint.addr);
    }

    bool additional_tx_queued = false;
    ethmac_netbuf_t* netbuf;
    zx_status_t send_status = ZX_OK;
    if (!list_is_empty(&ctx->tx_pending_infos)) {
        netbuf = list_peek_head_type(&ctx->tx_pending_infos, ethmac_netbuf_t, node);
        if ((send_status = send_locked(ctx, netbuf)) != ZX_ERR_SHOULD_WAIT) {
            list_remove_head(&ctx->tx_pending_infos);
            additional_tx_queued = true;
        }
    }

    mtx_unlock(&ctx->tx_mutex);

    mtx_lock(&ctx->ethmac_mutex);
    if (additional_tx_queued && ctx->ethmac_ifc) {
        ctx->ethmac_ifc->complete_tx(ctx->ethmac_cookie, netbuf, send_status);
    }
    mtx_unlock(&ctx->ethmac_mutex);

    // When the interface is offline, the transaction will complete with status set to
    // ZX_ERR_IO_NOT_PRESENT. There's not much we can do except ignore it.
}

// Note: the assumption made here is that no rx transmissions will be processed in parallel,
// so we do not maintain an rx mutex.
static void usb_recv(ecm_ctx_t* ctx, usb_request_t* request) {
    size_t len = request->response.actual;

    uint8_t* read_data;
    zx_status_t status = usb_req_mmap(&ctx->usb, request, (void*)&read_data);
    if (status != ZX_OK) {
        zxlogf(ERROR, "%s: usb_req_mmap failed with status %d\n",
                module_name, status);
        return;
    }

    mtx_lock(&ctx->ethmac_mutex);
    if (ctx->ethmac_ifc) {
        ctx->ethmac_ifc->recv(ctx->ethmac_cookie, read_data, len, 0);
    }
    mtx_unlock(&ctx->ethmac_mutex);
}

static void usb_read_complete(usb_request_t* request, void* cookie) {
    ecm_ctx_t* ctx = cookie;

    if (request->response.status != ZX_OK) {
        zxlogf(INFO, "%s: usb_read_complete called with status %d\n",
                module_name, (int)request->response.status);
    }

    if (request->response.status == ZX_ERR_IO_NOT_PRESENT) {
        usb_req_release(&ctx->usb, request);
        return;
    }

    if (request->response.status == ZX_ERR_IO_REFUSED) {
        zxlogf(INFO, "%s: resetting receive endpoint\n", module_name);
        usb_reset_endpoint(&ctx->usb, ctx->rx_endpoint.addr);
    } else if (request->response.status == ZX_ERR_IO_INVALID) {
        if (ctx->rx_endpoint_delay < ETHMAC_MAX_RECV_DELAY) {
            ctx->rx_endpoint_delay += ETHMAC_RECV_DELAY;
        }
        zxlogf(INFO, "%s: slowing down the requests by %d usec."
                     "Resetting the recv endpoint\n",
               module_name, ETHMAC_RECV_DELAY);
        usb_reset_endpoint(&ctx->usb, ctx->rx_endpoint.addr);
    } else if (request->response.status == ZX_OK) {
        usb_recv(ctx, request);
    }

    zx_nanosleep(zx_deadline_after(ZX_USEC(ctx->rx_endpoint_delay)));
    usb_request_queue(&ctx->usb, request);
}

// check what modem manager does to communicate in userland
// expose a ethernet device and a channel based device for the commands. 
static zx_status_t ethmac_queue_tx(void* cookie, uint32_t options, ethmac_netbuf_t* netbuf) {
    ecm_ctx_t* ctx = cookie;
    size_t length = netbuf->len;
    zx_status_t status;

    if (length > ctx->mtu || length == 0) {
        return ZX_ERR_INVALID_ARGS;
    }

    zxlogf(INFO, "%s: sending %zu bytes to endpoint 0x%"PRIx8"\n",
            module_name, length, ctx->tx_endpoint.addr);

    mtx_lock(&ctx->tx_mutex);
    if (ctx->unbound) {
        status = ZX_ERR_IO_NOT_PRESENT;
    } else {
        status = send_locked(ctx, netbuf);
        if (status == ZX_ERR_SHOULD_WAIT) {
            // No buffers available, queue it up
            list_add_tail(&ctx->tx_pending_infos, &netbuf->node);
        }
    }

    mtx_unlock(&ctx->tx_mutex);
    return status;
}

static zx_status_t ethmac_set_param(void *cookie, uint32_t param, int32_t value, void* data) {
    zxlogf(ERROR, "qmi: attempting to set param\n");
    return ZX_ERR_NOT_SUPPORTED;
}

static ethmac_protocol_ops_t ethmac_ops = {
    .query = ethmac_query,
    .stop = ethmac_stop,
    .start = ethmac_start,
    .queue_tx = ethmac_queue_tx,
    .set_param = ethmac_set_param,
};

static void qmi_interrupt_complete(usb_request_t* request, void* cookie) {
    zxlogf(ERROR, "qmi: got interrupt!\n");
    ecm_ctx_t* ctx = cookie;
    sync_completion_signal(&ctx->completion);
}

static void ecm_handle_interrupt(ecm_ctx_t* ctx, usb_request_t* request) {
    zxlogf(ERROR, "qmi: handling interruput!!!!!!!!!!!\n");
    if (request->response.actual < sizeof(usb_cdc_notification_t)) {
        zxlogf(ERROR, "%s: ignored interrupt (size = %ld)\n", module_name, (long)request->response.actual);
        return;
    }

    usb_cdc_notification_t usb_req;
    usb_req_copy_from(&ctx->usb, request, &usb_req, sizeof(usb_cdc_notification_t), 0);
    //if (usb_req.bmRequestType == (USB_DIR_IN | USB_TYPE_CLASS | USB_RECIP_INTERFACE) &&
    //    usb_req.bNotification == USB_CDC_NC_NETWORK_CONNECTION) {
    //    ecm_update_online_status(ctx, usb_req.wValue != 0);
    //} else if (usb_req.bmRequestType == (USB_DIR_IN | USB_TYPE_CLASS | USB_RECIP_INTERFACE) &&
    //           usb_req.bNotification == USB_CDC_NC_CONNECTION_SPEED_CHANGE) {
    //    // The ethermac driver doesn't care about speed changes, so even though we track this
    //    // information, it's currently unused.
    //    if (usb_req.wLength != 8) {
    //        zxlogf(ERROR, "%s: invalid size (%"PRIu16") for CONNECTION_SPEED_CHANGE notification\n",
    //               module_name, usb_req.wLength);
    //        return;
    //    }
    //    // Data immediately follows notification in packet
    //    uint32_t new_us_bps, new_ds_bps;
    //    usb_req_copy_from(&ctx->usb, request, &new_us_bps, 4, sizeof(usb_cdc_notification_t));
    //    usb_req_copy_from(&ctx->usb, request, &new_ds_bps, 4, sizeof(usb_cdc_notification_t) + 4);
    //    if (new_us_bps != ctx->us_bps) {
    //        zxlogf(ERROR, "%s: connection speed change... upstream bits/s: %"PRIu32"\n",
    //                module_name, new_us_bps);
    //        ctx->us_bps = new_us_bps;
    //    }
    //    if (new_ds_bps != ctx->ds_bps) {
    //        zxlogf(ERROR, "%s: connection speed change... downstream bits/s: %"PRIu32"\n",
    //                module_name, new_ds_bps);
    //        ctx->ds_bps = new_ds_bps;
    //    }
    //}  else {
    //    zxlogf(ERROR, "%s: ignored interrupt (type = %"PRIu8", request = %"PRIu8")\n",
    //           module_name, usb_req.bmRequestType, usb_req.bNotification);
    //    return;
//    }
}

static int ecm_int_handler_thread(void* cookie) {
    ecm_ctx_t* ctx = cookie;
    usb_request_t* txn = ctx->int_txn_buf;
    zxlogf(INFO, "%s thread handler\n", module_name);

    while (true) {
        sync_completion_reset(&ctx->completion);
        usb_request_queue(&ctx->usb, txn);
        zxlogf(INFO, "%s before sync wait!\n", module_name);
        sync_completion_wait(&ctx->completion, ZX_TIME_INFINITE);
        zxlogf(INFO, "%s after sync wait!\n", module_name);
        if (txn->response.status == ZX_OK) {
            ecm_handle_interrupt(ctx, txn);
        } else if (txn->response.status == ZX_ERR_PEER_CLOSED ||
                   txn->response.status == ZX_ERR_IO_NOT_PRESENT) {
            zxlogf(INFO, "%s: terminating interrupt handling thread\n", module_name);
            return txn->response.status;
        } else if (txn->response.status == ZX_ERR_IO_REFUSED ||
                   txn->response.status == ZX_ERR_IO_INVALID) {
            zxlogf(INFO, "%s: resetting interrupt endpoint\n", module_name);
            usb_reset_endpoint(&ctx->usb, ctx->int_endpoint.addr);
        } else {
            zxlogf(ERROR, "%s: error (%ld) waiting for interrupt - ignoring\n",
                   module_name, (long)txn->response.status);
        }
    }
}

static void copy_endpoint_info(ecm_endpoint_t* ep_info, usb_endpoint_descriptor_t* desc) {
    ep_info->addr = desc->bEndpointAddress;
    ep_info->max_packet_size = desc->wMaxPacketSize;
}

static zx_status_t ecm_bind(void* ctx, zx_device_t* device) {
    zxlogf(INFO, "%s: starting %s\n", module_name, __FUNCTION__);

    usb_protocol_t usb;
    zx_status_t result = device_get_protocol(device, ZX_PROTOCOL_USB, &usb);
    if (result != ZX_OK) {
        return result;
    }

    // Allocate context
    ecm_ctx_t* ecm_ctx = calloc(1, sizeof(ecm_ctx_t));
    if (!ecm_ctx) {
        zxlogf(ERROR, "%s: failed to allocate memory for USB CDC ECM driver\n", module_name);
        return ZX_ERR_NO_MEMORY;
    }

    // Initialize context
    ecm_ctx->usb_device = device;
    memcpy(&ecm_ctx->usb, &usb, sizeof(ecm_ctx->usb));
    list_initialize(&ecm_ctx->tx_txn_bufs);
    list_initialize(&ecm_ctx->tx_pending_infos);
    mtx_init(&ecm_ctx->ethmac_mutex, mtx_plain);
    mtx_init(&ecm_ctx->tx_mutex, mtx_plain);

    usb_desc_iter_t iter;
    result = usb_desc_iter_init(&usb, &iter);
    if (result < 0) {
        zxlogf(ERROR, "qmi: usb iterator failed: %s\n", zx_status_get_string(result));
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
        zxlogf(ERROR, "qmi: interface does not have the required 3 endpoints: %s\n", zx_status_get_string(result));
        usb_desc_iter_release(&iter);
        return ZX_ERR_NOT_SUPPORTED;
    }

    usb_endpoint_descriptor_t* int_ep = NULL;
    usb_endpoint_descriptor_t* tx_ep = NULL;
    usb_endpoint_descriptor_t* rx_ep = NULL;
    //usb_interface_descriptor_t* default_ifc = NULL;
    //usb_interface_descriptor_t* data_ifc = NULL;

    usb_descriptor_header_t* desc = usb_desc_iter_next(&iter);
    while (desc) {
    zxlogf(INFO, "qmi: Descriptor Type %d\n", desc->bDescriptorType);
      if (desc->bDescriptorType == USB_DT_ENDPOINT) {
        usb_endpoint_descriptor_t* endp = (void*)desc;
        if (usb_ep_direction(endp) == USB_ENDPOINT_OUT) {
            if (usb_ep_type(endp) == USB_ENDPOINT_BULK) {
                tx_ep = endp;
            }
        } else {
            if (usb_ep_type(endp) == USB_ENDPOINT_BULK) {
                rx_ep = endp;
            } else if (usb_ep_type(endp) == USB_ENDPOINT_INTERRUPT) {
                int_ep = endp;
            }
        }
      }
      desc = usb_desc_iter_next(&iter);
    }
    //usb_desc_iter_release(&iter);

    //if (cdc_header_desc == NULL || cdc_eth_desc == NULL) {
    //    zxlogf(ERROR, "%s: CDC %s descriptor(s) not found", module_name,
    //           cdc_header_desc ? "ethernet" : cdc_eth_desc ? "header" : "ethernet and header");
    //    goto fail;
    //}
    if (int_ep == NULL || tx_ep == NULL || rx_ep == NULL) {
        zxlogf(ERROR, "%s: missing one or more required endpoints\n", module_name);
        goto fail;
    }
    //if (default_ifc == NULL) {
    //    zxlogf(ERROR, "%s: unable to find CDC default interface\n", module_name);
    //    goto fail;
    //}
    //if (data_ifc == NULL) {
    //    zxlogf(ERROR, "%s: unable to find CDC data interface\n", module_name);
    //    goto fail;
    //}

    //// Parse the information in the CDC descriptors
    //if (!parse_cdc_header(cdc_header_desc)) {
    //    goto fail;
    //}
    //if (!parse_cdc_ethernet_descriptor(ecm_ctx, cdc_eth_desc)) {
    //    goto fail;
    //}
    zxlogf(ERROR, "qmi: Down here!\n");

    // Parse endpoint information
    copy_endpoint_info(&ecm_ctx->int_endpoint, int_ep);
    copy_endpoint_info(&ecm_ctx->tx_endpoint, tx_ep);
    copy_endpoint_info(&ecm_ctx->rx_endpoint, rx_ep);

    ecm_ctx->rx_endpoint_delay = ETHMAC_INITIAL_RECV_DELAY;
    ecm_ctx->tx_endpoint_delay = ETHMAC_INITIAL_TRANSMIT_DELAY;
    // Reset by selecting default interface followed by data interface. We can't start
    // queueing transactions until this is complete.
//    usb_set_interface(&usb, default_ifc->bInterfaceNumber, default_ifc->bAlternateSetting);
    zxlogf(ERROR, "qmi: set the interface?\n");
    usb_set_interface(&usb, 8, 0);
    zxlogf(ERROR, "qmi: set the interface done\n");

    // Allocate interrupt transaction buffer
    usb_request_t* int_buf;
    zx_status_t alloc_result = usb_req_alloc(&usb, &int_buf,
                                             ecm_ctx->int_endpoint.max_packet_size,
                                             ecm_ctx->int_endpoint.addr);
    if (alloc_result != ZX_OK) {
        result = alloc_result;
        goto fail;
    }

    int_buf->complete_cb = qmi_interrupt_complete;
    int_buf->cookie = ecm_ctx;
    ecm_ctx->int_txn_buf = int_buf;
    //zxlogf(ERROR, "qmi: set the interface ensteeeeeeeeeeeeeeeeeeedone\n");

    ecm_ctx->mtu = 512;

    // Allocate tx transaction buffers
    uint16_t tx_buf_sz = ecm_ctx->mtu;
#if MAX_TX_BUF_SZ < UINT16_MAX
    if (tx_buf_sz > MAX_TX_BUF_SZ) {
        zxlogf(ERROR, "%s: insufficient space for even a single tx buffer\n", module_name);
        goto fail;
    }
#endif
    size_t tx_buf_remain = MAX_TX_BUF_SZ;
    while (tx_buf_remain >= tx_buf_sz) {
        usb_request_t* tx_buf;
        zxlogf(ERROR, "qmi: allocing %d\n", (int)tx_buf_remain);
        zx_status_t alloc_result = usb_req_alloc(&usb, &tx_buf, tx_buf_sz,
                                                 ecm_ctx->tx_endpoint.addr);
        if (alloc_result != ZX_OK) {
            result = alloc_result;
            goto fail;
        }

        // As per the CDC-ECM spec, we need to send a zero-length packet to signify the end of
        // transmission when the endpoint max packet size is a factor of the total transmission size
        tx_buf->header.send_zlp = true;

        tx_buf->complete_cb = usb_write_complete;
        tx_buf->cookie = ecm_ctx;
        list_add_head(&ecm_ctx->tx_txn_bufs, &tx_buf->node);
        tx_buf_remain -= tx_buf_sz;
    }
    zxlogf(ERROR, "qmi: atnoehunsaoehtunsset the interface done\n");

    // Allocate rx transaction buffers
    uint16_t rx_buf_sz = ecm_ctx->mtu;
#if MAX_TX_BUF_SZ < UINT16_MAX
    if (rx_buf_sz > MAX_RX_BUF_SZ) {
        zxlogf(ERROR, "%s: insufficient space for even a single rx buffer\n", module_name);
        goto fail;
    }
#endif
    size_t rx_buf_remain = MAX_RX_BUF_SZ;
    while (rx_buf_remain >= rx_buf_sz) {
        usb_request_t* rx_buf;
        zx_status_t alloc_result = usb_req_alloc(&usb, &rx_buf, rx_buf_sz,
                                                 ecm_ctx->rx_endpoint.addr);
        if (alloc_result != ZX_OK) {
            result = alloc_result;
            goto fail;
        }

        rx_buf->complete_cb = usb_read_complete;
        rx_buf->cookie = ecm_ctx;
        usb_request_queue(&ecm_ctx->usb, rx_buf);
        rx_buf_remain -= rx_buf_sz;
    }

    zxlogf(ERROR, "qmi: Starting the thread!\n");
    // Kick off the handler thread
    int thread_result = thrd_create_with_name(&ecm_ctx->int_thread, ecm_int_handler_thread,
                                              ecm_ctx, "ecm_int_handler_thread");
    if (thread_result != thrd_success) {
        zxlogf(ERROR, "%s: failed to create interrupt handler thread (%d)\n", module_name, thread_result);
        goto fail;
    }

    // Add the device
    device_add_args_t args = {
        .version = DEVICE_ADD_ARGS_VERSION,
        .name = "qmi",
        .ctx = ecm_ctx,
        .ops = &ecm_device_proto,
        .proto_id = ZX_PROTOCOL_ETHERNET_IMPL,
        .proto_ops = &ethmac_ops,
    };
    result = device_add(ecm_ctx->usb_device, &args, &ecm_ctx->zxdev);
    if (result < 0) {
        zxlogf(ERROR, "%s: failed to add device: %d\n", module_name, (int)result);
        goto fail;
    }

    device_add_args_t qmi_args = {
      .version = DEVICE_ADD_ARGS_VERSION,
        .name = "qmi_transport",
        .ctx = ecm_ctx,
        .ops = &qmi_device_proto,
        .proto_id = ZX_PROTOCOL_QMI_TRANSPORT,
        //.props = props,
        //.prop_count = countof(props),
    };
    result = device_add(ecm_ctx->usb_device, &qmi_args, &ecm_ctx->zxdev);


    usb_desc_iter_release(&iter);
    return ZX_OK;

fail:
    usb_desc_iter_release(&iter);
    ecm_free(ecm_ctx);
    zxlogf(ERROR, "%s: failed to bind\n", module_name);
    return result;
}

static zx_driver_ops_t qmi_driver_ops = {
    .version = DRIVER_OPS_VERSION,
    .bind = ecm_bind,
};

// clang-format off
ZIRCON_DRIVER_BEGIN(qmi_usb, qmi_driver_ops, "zircon", "0.1", 3)
    BI_ABORT_IF(NE, BIND_PROTOCOL, ZX_PROTOCOL_USB),
    BI_ABORT_IF(NE, BIND_USB_VID, SIERRA_VID),
    BI_MATCH_IF(EQ, BIND_USB_PID, EM7565_PID),
ZIRCON_DRIVER_END(qmi_usb)
