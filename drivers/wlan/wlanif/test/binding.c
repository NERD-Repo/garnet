// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <stdlib.h>
#include <string.h>
#define _ALL_SOURCE
#include <threads.h>

#include <ddk/binding.h>
#include <ddk/device.h>
#include <ddk/driver.h>
#include <wlan/protocol/if-impl.h>
#include <zircon/types.h>

wlanif_impl_ifc_t wlanif_ifc = {};
void* wlanif_cookie = NULL;
zx_device_t* global_device;
uint64_t scan_txn_id;

zx_status_t wlanif_start(void* ctx, wlanif_impl_ifc_t* ifc, void* cookie) {
    printf("***** wlanif_start called\n");
    memcpy(&wlanif_ifc, ifc, sizeof(wlanif_ifc));
    wlanif_cookie = cookie;
    return ZX_OK;
}

void wlanif_stop(void* ctx) {
}

#define NUM_SCAN_RESULTS 12
#define MAX_SSID_LEN 100

#define INCREMENTAL_SCAN

void fake_scan_result(wlanif_bss_description_t* bss_desc) {
    static uint8_t chan = 1;
    memset(bss_desc, 0, sizeof(*bss_desc));
    static size_t scan_num = 3;

    // bssid
    memset(bss_desc->bssid, scan_num, ETH_ALEN);

    // ssid
    char* ssid = malloc(MAX_SSID_LEN);
    sprintf(ssid, "FAKE AP %d", (int)scan_num++);
    bss_desc->ssid = (const char*)ssid;

    // bss_type
    bss_desc->bss_type = BSS_TYPE_INFRASTRUCTURE;

    // rsne
    static uint8_t fake_rsne_data[] = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    bss_desc->rsne_len = countof(fake_rsne_data);
    bss_desc->rsne = fake_rsne_data;

    // chan
    bss_desc->chan.primary = chan;
    chan = (chan % 14) + 1;
    bss_desc->chan.cbw = CBW20;
    bss_desc->chan.secondary80 = 0;
}

void free_scan_result(wlanif_bss_description_t* bss_desc) {
    free((void*)bss_desc->ssid);
}

void fake_scan_end(void) {
    printf("***** faking scan complete\n");
    wlanif_scan_end_t args;
    args.txn_id = scan_txn_id;
    args.code = SCAN_RESULT_SUCCESS;
    wlanif_ifc.on_scan_end(wlanif_cookie, &args);
}

#ifdef INCREMENTAL_SCAN
int fake_scan_results(void* arg) {
    printf("***** faking scan results!\n");
    for (int iter = 0; iter < NUM_SCAN_RESULTS; iter++) {
        zx_nanosleep(zx_deadline_after(ZX_MSEC(200)));
        wlanif_scan_result_t scan_result;
        scan_result.txn_id = scan_txn_id;
        fake_scan_result(&scan_result.bss);
        wlanif_ifc.on_scan_result(wlanif_cookie, &scan_result);
        free_scan_result(&scan_result.bss);
    }
    zx_nanosleep(zx_deadline_after(ZX_MSEC(200)));
    wlanif_scan_end_t scan_end = {.txn_id = scan_txn_id,
                                  .code = SCAN_RESULT_SUCCESS};
    wlanif_ifc.on_scan_end(wlanif_cookie, &scan_end);
    return 0;
}
#else
int fake_scan_results(void* arg) {
    printf("***** faking scan results!\n");
    wlanif_bss_description_t bss_descs[NUM_SCAN_RESULTS];
    for (int iter = 0; iter < NUM_SCAN_RESULTS; iter++) {
        fake_scan_result(&bss_descs[iter]);
    }
    zx_nanosleep(zx_deadline_after(ZX_SEC(2)));
    wlanif_scan_confirm_t conf;
    conf.num_bss_descs = NUM_SCAN_RESULTS;
    conf.bss_description_set = bss_descs;
    conf.result_code = SCAN_RESULT_SUCCESS;
    wlanif_ifc.scan_conf(wlanif_cookie, &conf);
    for (int iter = 0; iter < NUM_SCAN_RESULTS; iter++) {
        free_scan_result(&bss_descs[iter]);
    }
    return 0;
}
#endif

void wlanif_start_scan(void* ctx, wlanif_scan_req_t* req) {
    printf("***** starting scan (txn_id = %lu)!!!\n", req->txn_id);
    scan_txn_id = req->txn_id;
    thrd_t scan_thrd;
    thrd_create_with_name(&scan_thrd, fake_scan_results, NULL, "wlanif-test-fake-scan");
    return;
}

void wlanif_join_req(void* ctx, wlanif_join_req_t* req) {
    printf("***** join_req\n");
}

void wlanif_auth_req(void* ctx, wlanif_auth_req_t* req) {
    printf("***** auth_req\n");
}

void wlanif_auth_ind(void* ctx, wlanif_auth_ind_t* ind) {
    printf("***** auth_ind\n");
}

void wlanif_deauth_req(void* ctx, wlanif_deauth_req_t* req) {
    printf("***** deauth_req\n");
}

void wlanif_assoc_req(void* ctx, wlanif_assoc_req_t* req) {
    printf("***** assoc_req\n");
}

void wlanif_assoc_ind(void* ctx, wlanif_assoc_ind_t* ind) {
    printf("***** assoc_ind\n");
}

void wlanif_disassoc_req(void* ctx, wlanif_disassoc_req_t* req) {
    printf("***** disassoc_req\n");
}

void wlanif_reset_req(void* ctx, wlanif_reset_req_t* req) {
    printf("***** reset_req\n");
}

void wlanif_start_req(void* ctx, wlanif_start_req_t* req) {
    printf("***** start_req\n");
}

void wlanif_stop_req(void* ctx, wlanif_stop_req_t* req) {
    printf("***** stop_req\n");
}

void wlanif_set_keys_req(void* ctx, wlanif_set_keys_req_t* req) {
    printf("***** set_keys_req\n");
}

void wlanif_del_keys_req(void* ctx, wlanif_del_keys_req_t* req) {
    printf("***** del_keys_req\n");
}

void wlanif_eapol_req(void* ctx, wlanif_eapol_req_t* req) {
    printf("***** eapol_req\n");
}

void wlanif_query(void* ctx, wlanif_query_info_t* info) {
    printf("***** query\n");
    memset(info, 0, sizeof(*info));
    uint8_t mac_addr[ETH_ALEN] = { 1, 2, 3, 4, 5, 6 };
    memcpy(info->mac_addr, mac_addr, ETH_ALEN);
    info->role = MAC_ROLE_CLIENT;
    info->features = 0;
    info->num_bands = 1;
    static uint16_t basic_rates[] = { 2, 4, 11, 22, 12, 18, 24, 36, 48, 72, 96, 108 };
    static uint8_t channels[] = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14 };
    static wlanif_band_capabilities_t band = {
        .num_basic_rates = countof(basic_rates),
        .basic_rates = basic_rates,
        .base_frequency = 2407,
        .num_channels = countof(channels),
        .channels = channels,
    }; 
    info->bands = &band;
}

zx_status_t wlanif_data_queue_tx(void* ctx, uint32_t options, ethmac_netbuf_t* netbuf) {
    printf("***** data_queue_tx\n");
    return ZX_OK;
}

wlanif_impl_protocol_ops_t wlanif_impl_ops = {
    .start = wlanif_start,
    .stop = wlanif_stop,
    .query = wlanif_query,
    .start_scan = wlanif_start_scan,
    .join_req = wlanif_join_req,
    .auth_req = wlanif_auth_req,
    .auth_ind = wlanif_auth_ind,
    .deauth_req = wlanif_deauth_req,
    .assoc_req = wlanif_assoc_req,
    .assoc_ind = wlanif_assoc_ind,
    .disassoc_req = wlanif_disassoc_req,
    .reset_req = wlanif_reset_req,
    .start_req = wlanif_start_req,
    .stop_req = wlanif_stop_req,
    .set_keys_req = wlanif_set_keys_req,
    .del_keys_req = wlanif_del_keys_req,
    .eapol_req = wlanif_eapol_req,
    .data_queue_tx = wlanif_data_queue_tx,
};

static zx_protocol_device_t device_ops = {
    .version = DEVICE_OPS_VERSION,
};

zx_status_t dev_bind(void* ctx, zx_device_t* device) {

    static bool first = true;

    if (! first) {
        return ZX_ERR_ALREADY_BOUND;
    }
    first = false;
    static device_add_args_t args = {
        .version = DEVICE_ADD_ARGS_VERSION,
        .name = "wlanif-test",
        .ctx = NULL,
        .ops = &device_ops,
        .proto_id = ZX_PROTOCOL_WLANIF_IMPL,
        .proto_ops = &wlanif_impl_ops,
    };
    return device_add(device, &args, &global_device);
}

zx_status_t dev_init(void** out_ctx) {
    return ZX_OK;
}

void dev_release(void* ctx) {
}

static zx_driver_ops_t wlanif_test_driver_ops = {
    .version = DRIVER_OPS_VERSION,
    .init = dev_init,
    .bind = dev_bind,
    .release = dev_release,
};

ZIRCON_DRIVER_BEGIN(wlanif-test, wlanif_test_driver_ops, "fuchsia", "0.1", 1)
BI_MATCH_IF(EQ, BIND_PROTOCOL, ZX_PROTOCOL_TEST_PARENT), ZIRCON_DRIVER_END(wlanif-test)
