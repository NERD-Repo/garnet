// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#pragma once

#include <ddk/protocol/ethernet.h>
#include <net/ethernet.h>
#include <wlan/protocol/info.h>
#include <zircon/compiler.h>

__BEGIN_CDECLS

enum wlanif_bss_types {
    BSS_TYPE_INFRASTRUCTURE = 1,
    BSS_TYPE_PERSONAL = 2,
    BSS_TYPE_INDEPENDENT = 3,
    BSS_TYPE_MESH = 4,
    BSS_TYPE_ANY_BSS = 5,
};

enum wlanif_scan_types {
    SCAN_TYPE_ACTIVE = 1,
    SCAN_TYPE_PASSIVE = 2,
};

typedef struct wlanif_scan_req {
    uint64_t txn_id;
    enum wlanif_bss_types bss_type;
    uint8_t bssid[ETH_ALEN];
    const char* ssid;
    enum wlanif_scan_types scan_type;
    uint32_t probe_delay;
    size_t num_channels;
    uint8_t* channel_list;
    uint32_t min_channel_time;
    uint32_t max_channel_time;
    size_t num_ssids;
    const char** ssid_list;
} wlanif_scan_req_t;

typedef struct wlanif_bss_description {
    uint8_t bssid[ETH_ALEN];
    const char* ssid;
    enum wlanif_bss_types bss_type;
    uint32_t beacon_period;
    uint32_t dtim_period;
    uint64_t timestamp;
    uint64_t local_time;
    size_t rsne_len;
    uint8_t* rsne;
    wlan_channel_t chan;
    int8_t rssi_dbm;
    int16_t rcpi_dbmh;
    int16_t rsni_dbh;
} wlanif_bss_description_t;

typedef struct wlanif_join_req {
    wlanif_bss_description_t selected_bss;
    uint32_t join_failure_timeout;
    uint32_t nav_sync_delay;
    size_t num_op_rates;
    uint16_t* op_rates;
} wlanif_join_req_t;

enum wlanif_auth_types {
    AUTH_TYPE_OPEN_SYSTEM = 1,
    AUTH_TYPE_SHARED_KEY = 2,
    AUTH_TYPE_FAST_BSS_TRANSITION = 3,
    AUTH_TYPE_SAE = 4,
};

typedef struct wlanif_auth_req {
    uint8_t peer_sta_address[ETH_ALEN];
    enum wlanif_auth_types auth_type;
    uint32_t auth_failure_timeout;
} wlanif_auth_req_t;

typedef struct wlanif_auth_ind {
    uint8_t peer_sta_address[ETH_ALEN];
    enum wlanif_auth_types auth_type;
} wlanif_auth_ind_t;

enum wlanif_deauth_reason_codes {
    // 0 Reserved
    DEAUTH_REASON_UNSPECIFIED = 1,
    DEAUTH_REASON_INVALID_AUTHENTICATION = 2,
    DEAUTH_REASON_LEAVING_NETWORK_DEAUTH = 3,
    DEAUTH_REASON_INACTIVITY = 4,
    DEAUTH_REASON_NO_MORE_STAS = 5,
    DEAUTH_REASON_INVALID_CLASS2_FRAME = 6,
    DEAUTH_REASON_INVALID_CLASS3_FRAME = 7,
    DEAUTH_REASON_LEAVING_NETWORK_DISASSOC = 8,
    DEAUTH_REASON_NOT_AUTHENTICATED = 9,
    DEAUTH_REASON_UNACCEPTABLE_POWER_CA = 10,
    DEAUTH_REASON_UNACCEPTABLE_SUPPORTED_CHANNELS = 11,
    DEAUTH_REASON_BSS_TRANSITION_DISASSOC = 12,
    DEAUTH_REASON_INVALID_ELEMENT = 13,
    DEAUTH_REASON_MIC_FAILURE = 14,
    DEAUTH_REASON_FOURWAY_HANDSHAKE_TIMEOUT = 15,
    DEAUTH_REASON_GK_HANDSHAKE_TIMEOUT = 16,
    DEAUTH_REASON_HANDSHAKE_ELEMENT_MISMATCH = 17,
    DEAUTH_REASON_INVALID_GROUP_CIPHER = 18,
    DEAUTH_REASON_INVALID_PAIRWISE_CIPHER = 19,
    DEAUTH_REASON_INVALID_AKMP = 20,
    DEAUTH_REASON_UNSUPPORTED_RSNE_VERSION = 21,
    DEAUTH_REASON_INVALID_RSNE_CAPABILITIES = 22,
    DEAUTH_REASON_IEEE802_1_X_AUTH_FAILED = 23,
    DEAUTH_REASON_CIPHER_OUT_OF_POLICY = 24,
    DEAUTH_REASON_TDLS_PEER_UNREACHABLE = 25,
    DEAUTH_REASON_TDLS_UNSPECIFIED = 26,
    DEAUTH_REASON_SSP_REQUESTED_DISASSOC = 27,
    DEAUTH_REASON_NO_SSP_ROAMING_AGREEMENT = 28,
    DEAUTH_REASON_BAD_CIPHER_OR_AKM = 29,
    DEAUTH_REASON_NOT_AUTHORIZED_THIS_LOCATION = 30,
    DEAUTH_REASON_SERVICE_CHANGE_PRECLUDES_TS = 31,
    DEAUTH_REASON_UNSPECIFIED_QOS = 32,
    DEAUTH_REASON_NOT_ENOUGH_BANDWIDTH = 33,
    DEAUTH_REASON_MISSING_ACKS = 34,
    DEAUTH_REASON_EXCEEDED_TXOP = 35,
    DEAUTH_REASON_STA_LEAVING = 36,
    // Values 37 and 38 are overloaded but should be clear from context.
    DEAUTH_REASON_END_TS_BA_DLS = 37,
    DEAUTH_REASON_UNKNOWN_TS_BA = 38,
    DEAUTH_REASON_TIMEOUT = 39,
    // 40-44 Reserved
    DEAUTH_REASON_PEERKEY_MISMATCH = 45,
    DEAUTH_REASON_PEER_INITIATED = 46,
    DEAUTH_REASON_AP_INITIATED = 47,
    DEAUTH_REASON_INVALID_FT_ACTION_FRAME_COUNT = 48,
    DEAUTH_REASON_INVALID_PMKID = 49,
    DEAUTH_REASON_INVALID_MDE = 50,
    DEAUTH_REASON_INVALID_FTE = 51,
    DEAUTH_REASON_MESH_PEERING_CANCELED = 52,
    DEAUTH_REASON_MESH_MAX_PEERS = 53,
    DEAUTH_REASON_MESH_CONFIGURATION_POLICY_VIOLATION = 54,
    DEAUTH_REASON_MESH_CLOSE_RCVD = 55,
    DEAUTH_REASON_MESH_MAX_RETRIES = 56,
    DEAUTH_REASON_MESH_CONFIRM_TIMEOUT = 57,
    DEAUTH_REASON_MESH_INVALID_GTK = 58,
    DEAUTH_REASON_MESH_INCONSISTENT_PARAMETERS = 59,
    DEAUTH_REASON_MESH_INVALID_SECURITY_CAPABILITY = 60,
    DEAUTH_REASON_MESH_PATH_ERROR_NO_PROXY_INFORMATION = 61,
    DEAUTH_REASON_MESH_PATH_ERROR_NO_FORWARDING_INFORMATION = 62,
    DEAUTH_REASON_MESH_PATH_ERROR_DESTINATION_UNREACHABLE = 63,
    DEAUTH_REASON_MAC_ADDRESS_ALREADY_EXISTS_IN_MBSS = 64,
    DEAUTH_REASON_MESH_CHANNEL_SWITCH_REGULATORY_REQUIREMENTS = 65,
    DEAUTH_REASON_MESH_CHANNEL_SWITCH_UNSPECIFIED = 66,
    // 67 - 65535 Reserved
};

typedef struct wlanif_deauth_req {
    uint8_t peer_sta_address[ETH_ALEN];
    enum wlanif_deauth_reason_codes reason_code;
} wlanif_deauth_req_t;

typedef struct wlanif_assoc_req {
    uint8_t peer_sta_address[ETH_ALEN];
    size_t rsne_len;
    uint8_t* rsne;
} wlanif_assoc_req_t;

typedef struct wlanif_assoc_ind {
    uint8_t peer_sta_address[ETH_ALEN];
    uint16_t listen_interval;
    size_t ssid_len;
    uint8_t* ssid;
    size_t rsne_len;
    uint8_t* rsne;
} wlanif_assoc_ind_t;

typedef struct wlanif_disassoc_req {
    uint8_t peer_sta_address[ETH_ALEN];
    uint16_t reason_code;
} wlanif_disassoc_req_t;

typedef struct wlanif_reset_req {
    uint8_t sta_address[ETH_ALEN];
    bool set_default_mib;
} wlanif_reset_req_t;

typedef struct wlanif_start_req {
    const char* ssid;
    enum wlanif_bss_types bss_type;
    uint32_t beacon_period;
    uint32_t dtim_period;
    uint8_t channel;
    size_t rsne_len;
    uint8_t* rsne;
} wlanif_start_req_t;

typedef struct wlanif_stop_req {
    const char* ssid;
} wlanif_stop_req_t;

enum wlanif_key_types {
    KEY_TYPE_GROUP = 1,
    KEY_TYPE_PAIRWISE = 2,
    KEY_TYPE_PEER_KEY = 3,
    KEY_TYPE_IGTK = 4,
};

typedef struct set_key_descriptor {
    uint8_t* key;
    uint16_t length;
    uint16_t key_id;
    enum wlanif_key_types key_type;
    uint8_t address[ETH_ALEN];
    uint8_t rsc[8];
    uint8_t cipher_suite_oui[3];
    uint8_t cipher_suite_type;
} set_key_descriptor_t;

typedef struct wlanif_set_keys_req {
    size_t num_keys;
    set_key_descriptor_t* keylist;
} wlanif_set_keys_req_t;

typedef struct delete_key_descriptor {
    uint16_t key_id;
    enum wlanif_key_types key_type;
    uint8_t address[ETH_ALEN];
} delete_key_descriptor_t;

typedef struct wlanif_del_keys_req {
    size_t num_keys;
    delete_key_descriptor_t* keylist;
} wlanif_del_keys_req_t;

typedef struct wlanif_eapol_req {
    uint8_t src_addr[ETH_ALEN];
    uint8_t dst_addr[ETH_ALEN];
    size_t data_len;
    uint8_t* data;
} wlanif_eapol_req_t;

typedef struct wlanif_scan_result {
    uint64_t txn_id;
    wlanif_bss_description_t bss;
} wlanif_scan_result_t;

enum wlanif_scan_result_codes {
    SCAN_RESULT_SUCCESS = 0,
    SCAN_RESULT_NOT_SUPPORTED = 1,
    SCAN_RESULT_INVALID_ARGS = 2,
    SCAN_RESULT_INTERNAL_ERROR = 3,
};

typedef struct wlanif_scan_end {
    uint64_t txn_id;
    enum wlanif_scan_result_codes code;
} wlanif_scan_end_t;

typedef struct wlanif_scan_confirm {
    size_t num_bss_descs;
    wlanif_bss_description_t* bss_description_set;
    enum wlanif_scan_result_codes result_code;
} wlanif_scan_confirm_t;

enum wlanif_join_result_codes {
    JOIN_RESULT_SUCCESS = 0,
    JOIN_RESULT_FAILURE_TIMEOUT = 1,
};

typedef struct wlanif_join_confirm {
    enum wlanif_join_result_codes result_code;
} wlanif_join_confirm_t;

enum wlanif_auth_result_codes {
    AUTH_RESULT_SUCCESS = 0,
    AUTH_RESULT_REFUSED = 1,
    AUTH_RESULT_ANTI_CLOGGING_TOKEN_REQUIRED = 2,
    AUTH_RESULT_FINITE_CYCLIC_GROUP_NOT_SUPPORTED = 3,
    AUTH_RESULT_REJECTED = 4,
    AUTH_RESULT_FAILURE_TIMEOUT = 5,
};

typedef struct wlanif_auth_confirm {
    uint8_t peer_sta_address[ETH_ALEN];
    enum wlanif_auth_types auth_type;
    enum wlanif_auth_result_codes result_code;
} wlanif_auth_confirm_t;

typedef struct wlanif_auth_resp {
    uint8_t peer_sta_address[ETH_ALEN];
    enum wlanif_auth_result_codes result_code;
} wlanif_auth_resp_t;

typedef struct wlanif_deauth_confirm {
    uint8_t peer_sta_address[ETH_ALEN];
} wlanif_deauth_confirm_t;

typedef struct wlanif_deauth_indication {
    uint8_t peer_sta_address[ETH_ALEN];
    enum wlanif_deauth_reason_codes reason_code;
} wlanif_deauth_indication_t;

enum wlanif_assoc_result_codes {
    ASSOC_RESULT_SUCCESS = 0,
    ASSOC_RESULT_REFUSED_REASON_UNSPECIFIED = 1,
    ASSOC_RESULT_REFUSED_NOT_AUTHENTICATED = 2,
    ASSOC_RESULT_REFUSED_CAPABILITIES_MISMATCH = 3,
    ASSOC_RESULT_REFUSED_EXTERNAL_REASON = 4,
    ASSOC_RESULT_REFUSED_AP_OUT_OF_MEMORY = 5,
    ASSOC_RESULT_REFUSED_BASIC_RATES_MISMATCH = 6,
    ASSOC_RESULT_REJECTED_EMERGENCY_SERVICES_NOT_SUPPORTED = 7,
    ASSOC_RESULT_REFUSED_TEMPORARILY = 8,
};

typedef struct wlanif_assoc_confirm {
    enum wlanif_assoc_result_codes result_code;
    uint16_t association_id;
} wlanif_assoc_confirm_t;

typedef struct wlanif_assoc_response {
    uint8_t peer_sta_address[ETH_ALEN];
    enum wlanif_assoc_result_codes result_code;
    uint16_t association_id;
} wlanif_assoc_response_t;

typedef struct wlanif_disassoc_confirm {
    int32_t status;
} wlanif_disassoc_confirm_t;

typedef struct wlanif_disassoc_indication {
    uint8_t peer_sta_address[ETH_ALEN];
    uint16_t reason_code;
} wlanif_disassoc_indication_t;

enum wlanif_start_result_codes {
    START_RESULT_SUCCESS = 0,
    START_RESULT_BSS_ALREADY_STARTED_OR_JOINED = 1,
    START_RESULT_RESET_REQUIRED_BEFORE_START = 2,
    START_RESULT_NOT_SUPPORTED = 3,
};

typedef struct wlanif_start_confirm {
    enum wlanif_start_result_codes result_code;
} wlanif_start_confirm_t;

enum wlanif_eapol_result_codes {
    EAPOL_RESULT_SUCCESS = 0,
    EAPOL_RESULT_TRANSMISSION_FAILURE = 1,
};

typedef struct wlanif_eapol_confirm {
    enum wlanif_eapol_result_codes result_code;
} wlanif_eapol_confirm_t;

typedef struct wlanif_signal_report_indication {
    int8_t rssi_dbm;
} wlanif_signal_report_indication_t;

typedef struct wlanif_eapol_indication {
    uint8_t src_addr[ETH_ALEN];
    uint8_t dst_addr[ETH_ALEN];
    size_t data_len;
    uint8_t* data;
} wlanif_eapol_indication_t;

enum mac_roles {
    MAC_ROLE_CLIENT = 1,
    MAC_ROLE_AP = 2,
};

typedef struct wlanif_band_capabilities {
    size_t num_basic_rates;
    uint16_t* basic_rates;
    uint16_t base_frequency;
    size_t num_channels;
    uint8_t* channels;
} wlanif_band_capabilities_t;

enum wlanif_features {
    WLANIF_FEATURE_DMA = 1UL << 0,    // Supports DMA buffer transfer protocol
    WLANIF_FEATURE_SYNTH = 1UL << 1,  // Synthetic (i.e., non-physical) device
};

typedef struct wlanif_query_info {
    uint8_t mac_addr[ETH_ALEN];
    enum mac_roles role;
    uint32_t features;
    size_t num_bands;
    wlanif_band_capabilities_t* bands;
} wlanif_query_info_t;

typedef struct wlanif_counter {
    uint64_t count;
    char* name;
} wlanif_counter_t;

typedef struct wlanif_packet_count {
    wlanif_counter_t in;
    wlanif_counter_t out;
    wlanif_counter_t drop;
} wlanif_packet_counter_t;

typedef struct wlanif_dispatcher_stats {
    wlanif_packet_counter_t any_packet;
    wlanif_packet_counter_t mgmt_frame;
    wlanif_packet_counter_t ctrl_frame;
    wlanif_packet_counter_t data_frame;
} wlanif_dispatcher_stats_t;

typedef struct wlanif_client_mlme_stats {
    wlanif_packet_counter_t svc_msg;
    wlanif_packet_counter_t data_frame;
    wlanif_packet_counter_t mgmt_frame;
} wlanif_client_mlme_stats_t;

typedef struct wlanif_ap_mlme_stats {
    wlanif_packet_counter_t not_used;
} wlanif_ap_mlme_stats_t;

typedef union wlanif_mlme_stats {
    wlanif_client_mlme_stats_t client_mlme_stats;
    wlanif_ap_mlme_stats_t ap_mlme_stats;
} wlanif_mlme_stats_t;

typedef struct wlanif_stats {
    wlanif_dispatcher_stats_t dispatcher_stats;
    wlanif_mlme_stats_t* mlme_stats;
} wlanif_stats_t;

typedef struct wlanif_stats_query_response {
    wlanif_stats_t stats;
} wlanif_stats_query_response_t;

typedef struct wlanif_impl_ifc {
    // MLME operations
    void (*on_scan_result)(void* cookie, wlanif_scan_result_t* result);
    void (*on_scan_end)(void* cookie, wlanif_scan_end_t* end);
    void (*scan_conf)(void* cookie, wlanif_scan_confirm_t* resp);
    void (*join_conf)(void* cookie, wlanif_join_confirm_t* resp);
    void (*auth_conf)(void* cookie, wlanif_auth_confirm_t* resp);
    void (*auth_resp)(void* cookie, wlanif_auth_resp_t* resp);
    void (*deauth_conf)(void* cookie, wlanif_deauth_confirm_t* resp);
    void (*deauth_ind)(void* cookie, wlanif_deauth_indication_t* ind);
    void (*assoc_conf)(void* cookie, wlanif_assoc_confirm_t* resp);
    void (*assoc_resp)(void* cookie, wlanif_assoc_response_t* resp);
    void (*disassoc_conf)(void* cookie, wlanif_disassoc_confirm_t* resp);
    void (*disassoc_ind)(void* cookie, wlanif_disassoc_indication_t* ind);
    void (*start_conf)(void* cookie, wlanif_start_confirm_t* resp);
    void (*stop_conf)(void* cookie);
    void (*eapol_conf)(void* cookie, wlanif_eapol_confirm_t* resp);

    // MLME extensions
    void (*signal_report)(void* cookie, wlanif_signal_report_indication_t* ind);
    void (*eapol_ind)(void* cookie, wlanif_eapol_indication_t* ind);
    void (*stats_query_resp)(void* cookie, wlanif_stats_query_response_t* resp);

    // Data operations
    void (*data_recv)(void* cookie, void* data, size_t length, uint32_t flags);
    void (*data_complete_tx)(void* cookie, ethmac_netbuf_t* netbuf, zx_status_t status);
} wlanif_impl_ifc_t;

typedef struct wlanif_impl_protocol_ops {
    // Lifecycle operations
    zx_status_t (*start)(void* ctx, wlanif_impl_ifc_t* ifc, void* cookie);
    void (*stop)(void* ctx);

    // State operation
    void (*query)(void* ctx, wlanif_query_info_t* info);

    // MLME operations
    void (*start_scan)(void* ctx, wlanif_scan_req_t* req);
    void (*join_req)(void* ctx, wlanif_join_req_t* req);
    void (*auth_req)(void* ctx, wlanif_auth_req_t* req);
    void (*auth_ind)(void* ctx, wlanif_auth_ind_t* ind);
    void (*deauth_req)(void* ctx, wlanif_deauth_req_t* req);
    void (*assoc_req)(void* ctx, wlanif_assoc_req_t* req);
    void (*assoc_ind)(void* ctx, wlanif_assoc_ind_t* ind);
    void (*disassoc_req)(void* ctx, wlanif_disassoc_req_t* req);
    void (*reset_req)(void* ctx, wlanif_reset_req_t* req);
    void (*start_req)(void* ctx, wlanif_start_req_t* req);
    void (*stop_req)(void* ctx, wlanif_stop_req_t* req);
    void (*set_keys_req)(void* ctx, wlanif_set_keys_req_t* req);
    void (*del_keys_req)(void* ctx, wlanif_del_keys_req_t* req);
    void (*eapol_req)(void* ctx, wlanif_eapol_req_t* req);

    // MLME extensions
    void (*stats_query_req)(void* ctx);

    // Data operations
    zx_status_t (*data_queue_tx)(void* ctx, uint32_t options, ethmac_netbuf_t* netbuf);

} wlanif_impl_protocol_ops_t;

typedef struct wlanif_impl_protocol {
    wlanif_impl_protocol_ops_t* ops;
    void* ctx;
} wlanif_impl_protocol_t;

__END_CDECLS
