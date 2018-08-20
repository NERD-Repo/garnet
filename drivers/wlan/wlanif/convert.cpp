// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "convert.h"

#include <net/ethernet.h>

namespace wlanif {

namespace wlan_mlme = ::fuchsia::wlan::mlme;

wlanif_bss_types ConvertBSSType(wlan_mlme::BSSTypes bss_type) {
    switch (bss_type) {
    case wlan_mlme::BSSTypes::INFRASTRUCTURE:
        return BSS_TYPE_INFRASTRUCTURE;
    case wlan_mlme::BSSTypes::PERSONAL:
        return BSS_TYPE_PERSONAL;
    case wlan_mlme::BSSTypes::INDEPENDENT:
        return BSS_TYPE_INDEPENDENT;
    case wlan_mlme::BSSTypes::MESH:
        return BSS_TYPE_MESH;
    case wlan_mlme::BSSTypes::ANY_BSS:
        return BSS_TYPE_ANY_BSS;
    default:
        ZX_ASSERT(0);
    }
}

wlanif_scan_types ConvertScanType(wlan_mlme::ScanTypes scan_type) {
    switch (scan_type) {
    case wlan_mlme::ScanTypes::ACTIVE:
        return SCAN_TYPE_ACTIVE;
    case wlan_mlme::ScanTypes::PASSIVE:
        return SCAN_TYPE_PASSIVE;
    default:
        ZX_ASSERT(0);
    }
}

uint8_t ConvertCBW(wlan_mlme::CBW cbw) {
    switch (cbw) {
    case wlan_mlme::CBW::CBW20:
        return CBW20;
    case wlan_mlme::CBW::CBW40:
        return CBW40;
    case wlan_mlme::CBW::CBW40BELOW:
        return CBW40BELOW;
    case wlan_mlme::CBW::CBW80:
        return CBW80;
    case wlan_mlme::CBW::CBW160:
        return CBW160;
    case wlan_mlme::CBW::CBW80P80:
        return CBW80P80;
    case wlan_mlme::CBW::CBW_COUNT:
        ZX_ASSERT(0);
    }
}

void ConvertWlanChan(wlan_channel_t* wlanif_chan, wlan_mlme::WlanChan* fidl_chan) {
    // primary
    wlanif_chan->primary = fidl_chan->primary;

    // CBW
    wlanif_chan->cbw = ConvertCBW(fidl_chan->cbw);

    // secondary80
    wlanif_chan->secondary80 = fidl_chan->secondary80;
}

void ConvertBSSDescription(wlanif_bss_description_t* wlanif_desc,
                           wlan_mlme::BSSDescription* fidl_desc) {
    // bssid
    std::memcpy(wlanif_desc->bssid, fidl_desc->bssid.data(), ETH_ALEN);

    // ssid
    wlanif_desc->ssid = fidl_desc->ssid->c_str();

    // bss_type
    wlanif_desc->bss_type = ConvertBSSType(fidl_desc->bss_type);

    // beacon_period
    wlanif_desc->beacon_period = fidl_desc->beacon_period;

    // dtim_period
    wlanif_desc->dtim_period = fidl_desc->dtim_period;

    // timestamp
    wlanif_desc->timestamp = fidl_desc->timestamp;

    // local_time
    wlanif_desc->local_time = fidl_desc->local_time;

    // rsne
    wlanif_desc->rsne_len = fidl_desc->rsn->size();
    wlanif_desc->rsne = fidl_desc->rsn->data();

    // chan
    ConvertWlanChan(&wlanif_desc->chan, &fidl_desc->chan);

    // rssi_dbm
    wlanif_desc->rssi_dbm = fidl_desc->rssi_dbm;

    // rcpi_dbmh
    wlanif_desc->rcpi_dbmh = fidl_desc->rcpi_dbmh;

    // rsni_dbh
    wlanif_desc->rsni_dbh = fidl_desc->rsni_dbh;
}

wlan_mlme::BSSTypes ConvertBSSType(wlanif_bss_types bss_type) {
    switch (bss_type) {
    case BSS_TYPE_INFRASTRUCTURE:
        return wlan_mlme::BSSTypes::INFRASTRUCTURE;
    case BSS_TYPE_PERSONAL:
        return wlan_mlme::BSSTypes::PERSONAL;
    case BSS_TYPE_INDEPENDENT:
        return wlan_mlme::BSSTypes::INDEPENDENT;
    case BSS_TYPE_MESH:
        return wlan_mlme::BSSTypes::MESH;
    case BSS_TYPE_ANY_BSS:
        return wlan_mlme::BSSTypes::ANY_BSS;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::CBW ConvertCBW(uint8_t cbw) {
    switch (cbw) {
    case CBW20:
        return wlan_mlme::CBW::CBW20;
    case CBW40:
        return wlan_mlme::CBW::CBW40;
    case CBW40BELOW:
        return wlan_mlme::CBW::CBW40BELOW;
    case CBW80:
        return wlan_mlme::CBW::CBW80;
    case CBW160:
        return wlan_mlme::CBW::CBW160;
    case CBW80P80:
        return wlan_mlme::CBW::CBW80P80;
    default:
        ZX_ASSERT(0);
    }
}

void ConvertWlanChan(wlan_mlme::WlanChan* fidl_chan, wlan_channel_t* wlanif_chan) {
    // primary
    fidl_chan->primary = wlanif_chan->primary;

    // CBW
    fidl_chan->cbw = ConvertCBW(wlanif_chan->cbw);

    // secondary80
    fidl_chan->secondary80 = wlanif_chan->secondary80;
}

template <typename T>
static void ArrayToVector(::fidl::VectorPtr<T>* vecptr, T* data, size_t len) {
    if (len > 0) {
        (*vecptr)->assign(data, data + len);
    }
}

void ConvertBSSDescription(wlan_mlme::BSSDescription* fidl_desc,
                           wlanif_bss_description_t* wlanif_desc) {
    // bssid
    std::memcpy(fidl_desc->bssid.mutable_data(), wlanif_desc->bssid, ETH_ALEN);

    // ssid
    fidl_desc->ssid = wlanif_desc->ssid;

    // bss_type
    fidl_desc->bss_type = ConvertBSSType(wlanif_desc->bss_type);

    // beacon_period
    fidl_desc->beacon_period = wlanif_desc->beacon_period;

    // dtim_period
    fidl_desc->dtim_period = wlanif_desc->dtim_period;

    // timestamp
    fidl_desc->timestamp = wlanif_desc->timestamp;

    // local_time
    fidl_desc->local_time = wlanif_desc->local_time;

    // rsne
    ArrayToVector(&fidl_desc->rsn, wlanif_desc->rsne, wlanif_desc->rsne_len);

    // chan
    ConvertWlanChan(&fidl_desc->chan, &wlanif_desc->chan);

    // rssi_dbm
    fidl_desc->rssi_dbm = wlanif_desc->rssi_dbm;

    // rcpi_dbmh
    fidl_desc->rcpi_dbmh = wlanif_desc->rcpi_dbmh;

    // rsni_dbh
    fidl_desc->rsni_dbh = wlanif_desc->rsni_dbh;
}

wlanif_auth_types ConvertAuthType(wlan_mlme::AuthenticationTypes auth_type) {
    switch (auth_type) {
    case wlan_mlme::AuthenticationTypes::OPEN_SYSTEM:
        return AUTH_TYPE_OPEN_SYSTEM;
    case wlan_mlme::AuthenticationTypes::SHARED_KEY:
        return AUTH_TYPE_SHARED_KEY;
    case wlan_mlme::AuthenticationTypes::FAST_BSS_TRANSITION:
        return AUTH_TYPE_FAST_BSS_TRANSITION;
    case wlan_mlme::AuthenticationTypes::SAE:
        return AUTH_TYPE_SAE;
    default:
        ZX_ASSERT(0);
    }
}

wlanif_deauth_reason_codes ConvertDeauthReasonCode(wlan_mlme::ReasonCode reason) {
    switch (reason) {
    case wlan_mlme::ReasonCode::UNSPECIFIED_REASON:
        return DEAUTH_REASON_UNSPECIFIED;
    case wlan_mlme::ReasonCode::INVALID_AUTHENTICATION:
        return DEAUTH_REASON_INVALID_AUTHENTICATION;
    case wlan_mlme::ReasonCode::LEAVING_NETWORK_DEAUTH:
        return DEAUTH_REASON_LEAVING_NETWORK_DEAUTH;
    case wlan_mlme::ReasonCode::REASON_INACTIVITY:
        return DEAUTH_REASON_INACTIVITY;
    case wlan_mlme::ReasonCode::NO_MORE_STAS:
        return DEAUTH_REASON_NO_MORE_STAS;
    case wlan_mlme::ReasonCode::INVALID_CLASS2_FRAME:
        return DEAUTH_REASON_INVALID_CLASS2_FRAME;
    case wlan_mlme::ReasonCode::INVALID_CLASS3_FRAME:
        return DEAUTH_REASON_INVALID_CLASS3_FRAME;
    case wlan_mlme::ReasonCode::LEAVING_NETWORK_DISASSOC:
        return DEAUTH_REASON_LEAVING_NETWORK_DISASSOC;
    case wlan_mlme::ReasonCode::NOT_AUTHENTICATED:
        return DEAUTH_REASON_NOT_AUTHENTICATED;
    case wlan_mlme::ReasonCode::UNACCEPTABLE_POWER_CA:
        return DEAUTH_REASON_UNACCEPTABLE_POWER_CA;
    case wlan_mlme::ReasonCode::UNACCEPTABLE_SUPPORTED_CHANNELS:
        return DEAUTH_REASON_UNACCEPTABLE_SUPPORTED_CHANNELS;
    case wlan_mlme::ReasonCode::BSS_TRANSITION_DISASSOC:
        return DEAUTH_REASON_BSS_TRANSITION_DISASSOC;
    case wlan_mlme::ReasonCode::REASON_INVALID_ELEMENT:
        return DEAUTH_REASON_INVALID_ELEMENT;
    case wlan_mlme::ReasonCode::MIC_FAILURE:
        return DEAUTH_REASON_MIC_FAILURE;
    case wlan_mlme::ReasonCode::FOURWAY_HANDSHAKE_TIMEOUT:
        return DEAUTH_REASON_FOURWAY_HANDSHAKE_TIMEOUT;
    case wlan_mlme::ReasonCode::GK_HANDSHAKE_TIMEOUT:
        return DEAUTH_REASON_GK_HANDSHAKE_TIMEOUT;
    case wlan_mlme::ReasonCode::HANDSHAKE_ELEMENT_MISMATCH:
        return DEAUTH_REASON_HANDSHAKE_ELEMENT_MISMATCH;
    case wlan_mlme::ReasonCode::REASON_INVALID_GROUP_CIPHER:
        return DEAUTH_REASON_INVALID_GROUP_CIPHER;
    case wlan_mlme::ReasonCode::REASON_INVALID_PAIRWISE_CIPHER:
        return DEAUTH_REASON_INVALID_PAIRWISE_CIPHER;
    case wlan_mlme::ReasonCode::REASON_INVALID_AKMP:
        return DEAUTH_REASON_INVALID_AKMP;
    case wlan_mlme::ReasonCode::UNSUPPORTED_RSNE_VERSION:
        return DEAUTH_REASON_UNSUPPORTED_RSNE_VERSION;
    case wlan_mlme::ReasonCode::INVALID_RSNE_CAPABILITIES:
        return DEAUTH_REASON_INVALID_RSNE_CAPABILITIES;
    case wlan_mlme::ReasonCode::IEEE802_1_X_AUTH_FAILED:
        return DEAUTH_REASON_IEEE802_1_X_AUTH_FAILED;
    case wlan_mlme::ReasonCode::REASON_CIPHER_OUT_OF_POLICY:
        return DEAUTH_REASON_CIPHER_OUT_OF_POLICY;
    case wlan_mlme::ReasonCode::TDLS_PEER_UNREACHABLE:
        return DEAUTH_REASON_TDLS_PEER_UNREACHABLE;
    case wlan_mlme::ReasonCode::TDLS_UNSPECIFIED_REASON:
        return DEAUTH_REASON_TDLS_UNSPECIFIED;
    case wlan_mlme::ReasonCode::SSP_REQUESTED_DISASSOC:
        return DEAUTH_REASON_SSP_REQUESTED_DISASSOC;
    case wlan_mlme::ReasonCode::NO_SSP_ROAMING_AGREEMENT:
        return DEAUTH_REASON_NO_SSP_ROAMING_AGREEMENT;
    case wlan_mlme::ReasonCode::BAD_CIPHER_OR_AKM:
        return DEAUTH_REASON_BAD_CIPHER_OR_AKM;
    case wlan_mlme::ReasonCode::NOT_AUTHORIZED_THIS_LOCATION:
        return DEAUTH_REASON_NOT_AUTHORIZED_THIS_LOCATION;
    case wlan_mlme::ReasonCode::SERVICE_CHANGE_PRECLUDES_TS:
        return DEAUTH_REASON_SERVICE_CHANGE_PRECLUDES_TS;
    case wlan_mlme::ReasonCode::UNSPECIFIED_QOS_REASON:
        return DEAUTH_REASON_UNSPECIFIED_QOS;
    case wlan_mlme::ReasonCode::NOT_ENOUGH_BANDWIDTH:
        return DEAUTH_REASON_NOT_ENOUGH_BANDWIDTH;
    case wlan_mlme::ReasonCode::MISSING_ACKS:
        return DEAUTH_REASON_MISSING_ACKS;
    case wlan_mlme::ReasonCode::EXCEEDED_TXOP:
        return DEAUTH_REASON_EXCEEDED_TXOP;
    case wlan_mlme::ReasonCode::STA_LEAVING:
        return DEAUTH_REASON_STA_LEAVING;
    case wlan_mlme::ReasonCode::END_TS_BA_DLS:
        return DEAUTH_REASON_END_TS_BA_DLS;
    case wlan_mlme::ReasonCode::UNKNOWN_TS_BA:
        return DEAUTH_REASON_UNKNOWN_TS_BA;
    case wlan_mlme::ReasonCode::TIMEOUT:
        return DEAUTH_REASON_TIMEOUT;
    case wlan_mlme::ReasonCode::PEERKEY_MISMATCH:
        return DEAUTH_REASON_PEERKEY_MISMATCH;
    case wlan_mlme::ReasonCode::PEER_INITIATED:
        return DEAUTH_REASON_PEER_INITIATED;
    case wlan_mlme::ReasonCode::AP_INITIATED:
        return DEAUTH_REASON_AP_INITIATED;
    case wlan_mlme::ReasonCode::REASON_INVALID_FT_ACTION_FRAME_COUNT:
        return DEAUTH_REASON_INVALID_FT_ACTION_FRAME_COUNT;
    case wlan_mlme::ReasonCode::REASON_INVALID_PMKID:
        return DEAUTH_REASON_INVALID_PMKID;
    case wlan_mlme::ReasonCode::REASON_INVALID_MDE:
        return DEAUTH_REASON_INVALID_MDE;
    case wlan_mlme::ReasonCode::REASON_INVALID_FTE:
        return DEAUTH_REASON_INVALID_FTE;
    case wlan_mlme::ReasonCode::MESH_PEERING_CANCELED:
        return DEAUTH_REASON_MESH_PEERING_CANCELED;
    case wlan_mlme::ReasonCode::MESH_MAX_PEERS:
        return DEAUTH_REASON_MESH_MAX_PEERS;
    case wlan_mlme::ReasonCode::MESH_CONFIGURATION_POLICY_VIOLATION:
        return DEAUTH_REASON_MESH_CONFIGURATION_POLICY_VIOLATION;
    case wlan_mlme::ReasonCode::MESH_CLOSE_RCVD:
        return DEAUTH_REASON_MESH_CLOSE_RCVD;
    case wlan_mlme::ReasonCode::MESH_MAX_RETRIES:
        return DEAUTH_REASON_MESH_MAX_RETRIES;
    case wlan_mlme::ReasonCode::MESH_CONFIRM_TIMEOUT:
        return DEAUTH_REASON_MESH_CONFIRM_TIMEOUT;
    case wlan_mlme::ReasonCode::MESH_INVALID_GTK:
        return DEAUTH_REASON_MESH_INVALID_GTK;
    case wlan_mlme::ReasonCode::MESH_INCONSISTENT_PARAMETERS:
        return DEAUTH_REASON_MESH_INCONSISTENT_PARAMETERS;
    case wlan_mlme::ReasonCode::MESH_INVALID_SECURITY_CAPABILITY:
        return DEAUTH_REASON_MESH_INVALID_SECURITY_CAPABILITY;
    case wlan_mlme::ReasonCode::MESH_PATH_ERROR_NO_PROXY_INFORMATION:
        return DEAUTH_REASON_MESH_PATH_ERROR_NO_PROXY_INFORMATION;
    case wlan_mlme::ReasonCode::MESH_PATH_ERROR_NO_FORWARDING_INFORMATION:
        return DEAUTH_REASON_MESH_PATH_ERROR_NO_FORWARDING_INFORMATION;
    case wlan_mlme::ReasonCode::MESH_PATH_ERROR_DESTINATION_UNREACHABLE:
        return DEAUTH_REASON_MESH_PATH_ERROR_DESTINATION_UNREACHABLE;
    case wlan_mlme::ReasonCode::MAC_ADDRESS_ALREADY_EXISTS_IN_MBSS:
        return DEAUTH_REASON_MAC_ADDRESS_ALREADY_EXISTS_IN_MBSS;
    case wlan_mlme::ReasonCode::MESH_CHANNEL_SWITCH_REGULATORY_REQUIREMENTS:
        return DEAUTH_REASON_MESH_CHANNEL_SWITCH_REGULATORY_REQUIREMENTS;
    case wlan_mlme::ReasonCode::MESH_CHANNEL_SWITCH_UNSPECIFIED:
        return DEAUTH_REASON_MESH_CHANNEL_SWITCH_UNSPECIFIED;
    default:
        ZX_ASSERT(0);
    }
}

wlanif_key_types ConvertKeyType(wlan_mlme::KeyType key_type) {
    switch (key_type) {
    case wlan_mlme::KeyType::GROUP:
        return KEY_TYPE_GROUP;
    case wlan_mlme::KeyType::PAIRWISE:
        return KEY_TYPE_PAIRWISE;
    case wlan_mlme::KeyType::PEER_KEY:
        return KEY_TYPE_PEER_KEY;
    case wlan_mlme::KeyType::IGTK:
        return KEY_TYPE_IGTK;
    default:
        ZX_ASSERT(0);
    }
}

void ConvertSetKeyDescriptor(set_key_descriptor_t* key_desc,
                             wlan_mlme::SetKeyDescriptor* fidl_key_desc) {
    // key
    key_desc->key = fidl_key_desc->key->data();

    // length
    key_desc->length = fidl_key_desc->key->size();

    // key_id
    key_desc->key_id = fidl_key_desc->key_id;

    // key_type
    key_desc->key_type = ConvertKeyType(fidl_key_desc->key_type);

    // address
    std::memcpy(key_desc->address, fidl_key_desc->address.data(), ETH_ALEN);

    // rsc
    std::memcpy(key_desc->rsc, fidl_key_desc->rsc.data(), sizeof(key_desc->rsc));

    // cipher_suite_oui
    std::memcpy(key_desc->cipher_suite_oui, fidl_key_desc->cipher_suite_oui.data(),
                sizeof(key_desc->cipher_suite_oui));

    // cipher_suite_type
    key_desc->cipher_suite_type = fidl_key_desc->cipher_suite_type;
}

void ConvertDeleteKeyDescriptor(delete_key_descriptor_t* key_desc,
                                wlan_mlme::DeleteKeyDescriptor* fidl_key_desc) {
    // key_id
    key_desc->key_id = fidl_key_desc->key_id;

    // key_type
    key_desc->key_type = ConvertKeyType(fidl_key_desc->key_type);

    // address
    std::memcpy(key_desc->address, fidl_key_desc->address.data(), ETH_ALEN);
}

wlan_mlme::ScanResultCodes ConvertScanResultCode(wlanif_scan_result_codes code) {
    switch (code) {
    case SCAN_RESULT_SUCCESS:
        return wlan_mlme::ScanResultCodes::SUCCESS;
    case SCAN_RESULT_NOT_SUPPORTED:
        return wlan_mlme::ScanResultCodes::NOT_SUPPORTED;
    case SCAN_RESULT_INVALID_ARGS:
        return wlan_mlme::ScanResultCodes::INVALID_ARGS;
    case SCAN_RESULT_INTERNAL_ERROR:
        return wlan_mlme::ScanResultCodes::INTERNAL_ERROR;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::JoinResultCodes ConvertJoinResultCode(wlanif_join_result_codes code) {
    switch (code) {
    case JOIN_RESULT_SUCCESS:
        return wlan_mlme::JoinResultCodes::SUCCESS;
    case JOIN_RESULT_FAILURE_TIMEOUT:
        return wlan_mlme::JoinResultCodes::JOIN_FAILURE_TIMEOUT;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::AuthenticationTypes ConvertAuthType(wlanif_auth_types auth_type) {
    switch (auth_type) {
    case AUTH_TYPE_OPEN_SYSTEM:
        return wlan_mlme::AuthenticationTypes::OPEN_SYSTEM;
    case AUTH_TYPE_SHARED_KEY:
        return wlan_mlme::AuthenticationTypes::SHARED_KEY;
    case AUTH_TYPE_FAST_BSS_TRANSITION:
        return wlan_mlme::AuthenticationTypes::FAST_BSS_TRANSITION;
    case AUTH_TYPE_SAE:
        return wlan_mlme::AuthenticationTypes::SAE;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::AuthenticateResultCodes ConvertAuthResultCode(wlanif_auth_result_codes code) {
    switch (code) {
    case AUTH_RESULT_SUCCESS:
        return wlan_mlme::AuthenticateResultCodes::SUCCESS;
    case AUTH_RESULT_REFUSED:
        return wlan_mlme::AuthenticateResultCodes::REFUSED;
    case AUTH_RESULT_ANTI_CLOGGING_TOKEN_REQUIRED:
        return wlan_mlme::AuthenticateResultCodes::ANTI_CLOGGING_TOKEN_REQUIRED;
    case AUTH_RESULT_FINITE_CYCLIC_GROUP_NOT_SUPPORTED:
        return wlan_mlme::AuthenticateResultCodes::FINITE_CYCLIC_GROUP_NOT_SUPPORTED;
    case AUTH_RESULT_REJECTED:
        return wlan_mlme::AuthenticateResultCodes::AUTHENTICATION_REJECTED;
    case AUTH_RESULT_FAILURE_TIMEOUT:
        return wlan_mlme::AuthenticateResultCodes::AUTH_FAILURE_TIMEOUT;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::ReasonCode ConvertDeauthReasonCode(wlanif_deauth_reason_codes reason) {
    switch (reason) {
    case DEAUTH_REASON_UNSPECIFIED:
        return wlan_mlme::ReasonCode::UNSPECIFIED_REASON;
    case DEAUTH_REASON_INVALID_AUTHENTICATION:
        return wlan_mlme::ReasonCode::INVALID_AUTHENTICATION;
    case DEAUTH_REASON_LEAVING_NETWORK_DEAUTH:
        return wlan_mlme::ReasonCode::LEAVING_NETWORK_DEAUTH;
    case DEAUTH_REASON_INACTIVITY:
        return wlan_mlme::ReasonCode::REASON_INACTIVITY;
    case DEAUTH_REASON_NO_MORE_STAS:
        return wlan_mlme::ReasonCode::NO_MORE_STAS;
    case DEAUTH_REASON_INVALID_CLASS2_FRAME:
        return wlan_mlme::ReasonCode::INVALID_CLASS2_FRAME;
    case DEAUTH_REASON_INVALID_CLASS3_FRAME:
        return wlan_mlme::ReasonCode::INVALID_CLASS3_FRAME;
    case DEAUTH_REASON_LEAVING_NETWORK_DISASSOC:
        return wlan_mlme::ReasonCode::LEAVING_NETWORK_DISASSOC;
    case DEAUTH_REASON_NOT_AUTHENTICATED:
        return wlan_mlme::ReasonCode::NOT_AUTHENTICATED;
    case DEAUTH_REASON_UNACCEPTABLE_POWER_CA:
        return wlan_mlme::ReasonCode::UNACCEPTABLE_POWER_CA;
    case DEAUTH_REASON_UNACCEPTABLE_SUPPORTED_CHANNELS:
        return wlan_mlme::ReasonCode::UNACCEPTABLE_SUPPORTED_CHANNELS;
    case DEAUTH_REASON_BSS_TRANSITION_DISASSOC:
        return wlan_mlme::ReasonCode::BSS_TRANSITION_DISASSOC;
    case DEAUTH_REASON_INVALID_ELEMENT:
        return wlan_mlme::ReasonCode::REASON_INVALID_ELEMENT;
    case DEAUTH_REASON_MIC_FAILURE:
        return wlan_mlme::ReasonCode::MIC_FAILURE;
    case DEAUTH_REASON_FOURWAY_HANDSHAKE_TIMEOUT:
        return wlan_mlme::ReasonCode::FOURWAY_HANDSHAKE_TIMEOUT;
    case DEAUTH_REASON_GK_HANDSHAKE_TIMEOUT:
        return wlan_mlme::ReasonCode::GK_HANDSHAKE_TIMEOUT;
    case DEAUTH_REASON_HANDSHAKE_ELEMENT_MISMATCH:
        return wlan_mlme::ReasonCode::HANDSHAKE_ELEMENT_MISMATCH;
    case DEAUTH_REASON_INVALID_GROUP_CIPHER:
        return wlan_mlme::ReasonCode::REASON_INVALID_GROUP_CIPHER;
    case DEAUTH_REASON_INVALID_PAIRWISE_CIPHER:
        return wlan_mlme::ReasonCode::REASON_INVALID_PAIRWISE_CIPHER;
    case DEAUTH_REASON_INVALID_AKMP:
        return wlan_mlme::ReasonCode::REASON_INVALID_AKMP;
    case DEAUTH_REASON_UNSUPPORTED_RSNE_VERSION:
        return wlan_mlme::ReasonCode::UNSUPPORTED_RSNE_VERSION;
    case DEAUTH_REASON_INVALID_RSNE_CAPABILITIES:
        return wlan_mlme::ReasonCode::INVALID_RSNE_CAPABILITIES;
    case DEAUTH_REASON_IEEE802_1_X_AUTH_FAILED:
        return wlan_mlme::ReasonCode::IEEE802_1_X_AUTH_FAILED;
    case DEAUTH_REASON_CIPHER_OUT_OF_POLICY:
        return wlan_mlme::ReasonCode::REASON_CIPHER_OUT_OF_POLICY;
    case DEAUTH_REASON_TDLS_PEER_UNREACHABLE:
        return wlan_mlme::ReasonCode::TDLS_PEER_UNREACHABLE;
    case DEAUTH_REASON_TDLS_UNSPECIFIED:
        return wlan_mlme::ReasonCode::TDLS_UNSPECIFIED_REASON;
    case DEAUTH_REASON_SSP_REQUESTED_DISASSOC:
        return wlan_mlme::ReasonCode::SSP_REQUESTED_DISASSOC;
    case DEAUTH_REASON_NO_SSP_ROAMING_AGREEMENT:
        return wlan_mlme::ReasonCode::NO_SSP_ROAMING_AGREEMENT;
    case DEAUTH_REASON_BAD_CIPHER_OR_AKM:
        return wlan_mlme::ReasonCode::BAD_CIPHER_OR_AKM;
    case DEAUTH_REASON_NOT_AUTHORIZED_THIS_LOCATION:
        return wlan_mlme::ReasonCode::NOT_AUTHORIZED_THIS_LOCATION;
    case DEAUTH_REASON_SERVICE_CHANGE_PRECLUDES_TS:
        return wlan_mlme::ReasonCode::SERVICE_CHANGE_PRECLUDES_TS;
    case DEAUTH_REASON_UNSPECIFIED_QOS:
        return wlan_mlme::ReasonCode::UNSPECIFIED_QOS_REASON;
    case DEAUTH_REASON_NOT_ENOUGH_BANDWIDTH:
        return wlan_mlme::ReasonCode::NOT_ENOUGH_BANDWIDTH;
    case DEAUTH_REASON_MISSING_ACKS:
        return wlan_mlme::ReasonCode::MISSING_ACKS;
    case DEAUTH_REASON_EXCEEDED_TXOP:
        return wlan_mlme::ReasonCode::EXCEEDED_TXOP;
    case DEAUTH_REASON_STA_LEAVING:
        return wlan_mlme::ReasonCode::STA_LEAVING;
    case DEAUTH_REASON_END_TS_BA_DLS:
        return wlan_mlme::ReasonCode::END_TS_BA_DLS;
    case DEAUTH_REASON_UNKNOWN_TS_BA:
        return wlan_mlme::ReasonCode::UNKNOWN_TS_BA;
    case DEAUTH_REASON_TIMEOUT:
        return wlan_mlme::ReasonCode::TIMEOUT;
    case DEAUTH_REASON_PEERKEY_MISMATCH:
        return wlan_mlme::ReasonCode::PEERKEY_MISMATCH;
    case DEAUTH_REASON_PEER_INITIATED:
        return wlan_mlme::ReasonCode::PEER_INITIATED;
    case DEAUTH_REASON_AP_INITIATED:
        return wlan_mlme::ReasonCode::AP_INITIATED;
    case DEAUTH_REASON_INVALID_FT_ACTION_FRAME_COUNT:
        return wlan_mlme::ReasonCode::REASON_INVALID_FT_ACTION_FRAME_COUNT;
    case DEAUTH_REASON_INVALID_PMKID:
        return wlan_mlme::ReasonCode::REASON_INVALID_PMKID;
    case DEAUTH_REASON_INVALID_MDE:
        return wlan_mlme::ReasonCode::REASON_INVALID_MDE;
    case DEAUTH_REASON_INVALID_FTE:
        return wlan_mlme::ReasonCode::REASON_INVALID_FTE;
    case DEAUTH_REASON_MESH_PEERING_CANCELED:
        return wlan_mlme::ReasonCode::MESH_PEERING_CANCELED;
    case DEAUTH_REASON_MESH_MAX_PEERS:
        return wlan_mlme::ReasonCode::MESH_MAX_PEERS;
    case DEAUTH_REASON_MESH_CONFIGURATION_POLICY_VIOLATION:
        return wlan_mlme::ReasonCode::MESH_CONFIGURATION_POLICY_VIOLATION;
    case DEAUTH_REASON_MESH_CLOSE_RCVD:
        return wlan_mlme::ReasonCode::MESH_CLOSE_RCVD;
    case DEAUTH_REASON_MESH_MAX_RETRIES:
        return wlan_mlme::ReasonCode::MESH_MAX_RETRIES;
    case DEAUTH_REASON_MESH_CONFIRM_TIMEOUT:
        return wlan_mlme::ReasonCode::MESH_CONFIRM_TIMEOUT;
    case DEAUTH_REASON_MESH_INVALID_GTK:
        return wlan_mlme::ReasonCode::MESH_INVALID_GTK;
    case DEAUTH_REASON_MESH_INCONSISTENT_PARAMETERS:
        return wlan_mlme::ReasonCode::MESH_INCONSISTENT_PARAMETERS;
    case DEAUTH_REASON_MESH_INVALID_SECURITY_CAPABILITY:
        return wlan_mlme::ReasonCode::MESH_INVALID_SECURITY_CAPABILITY;
    case DEAUTH_REASON_MESH_PATH_ERROR_NO_PROXY_INFORMATION:
        return wlan_mlme::ReasonCode::MESH_PATH_ERROR_NO_PROXY_INFORMATION;
    case DEAUTH_REASON_MESH_PATH_ERROR_NO_FORWARDING_INFORMATION:
        return wlan_mlme::ReasonCode::MESH_PATH_ERROR_NO_FORWARDING_INFORMATION;
    case DEAUTH_REASON_MESH_PATH_ERROR_DESTINATION_UNREACHABLE:
        return wlan_mlme::ReasonCode::MESH_PATH_ERROR_DESTINATION_UNREACHABLE;
    case DEAUTH_REASON_MAC_ADDRESS_ALREADY_EXISTS_IN_MBSS:
        return wlan_mlme::ReasonCode::MAC_ADDRESS_ALREADY_EXISTS_IN_MBSS;
    case DEAUTH_REASON_MESH_CHANNEL_SWITCH_REGULATORY_REQUIREMENTS:
        return wlan_mlme::ReasonCode::MESH_CHANNEL_SWITCH_REGULATORY_REQUIREMENTS;
    case DEAUTH_REASON_MESH_CHANNEL_SWITCH_UNSPECIFIED:
        return wlan_mlme::ReasonCode::MESH_CHANNEL_SWITCH_UNSPECIFIED;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::AssociateResultCodes ConvertAssocResultCode(wlanif_assoc_result_codes code) {
    switch (code) {
    case ASSOC_RESULT_SUCCESS:
        return wlan_mlme::AssociateResultCodes::SUCCESS;
    case ASSOC_RESULT_REFUSED_REASON_UNSPECIFIED:
        return wlan_mlme::AssociateResultCodes::REFUSED_REASON_UNSPECIFIED;
    case ASSOC_RESULT_REFUSED_NOT_AUTHENTICATED:
        return wlan_mlme::AssociateResultCodes::REFUSED_NOT_AUTHENTICATED;
    case ASSOC_RESULT_REFUSED_CAPABILITIES_MISMATCH:
        return wlan_mlme::AssociateResultCodes::REFUSED_CAPABILITIES_MISMATCH;
    case ASSOC_RESULT_REFUSED_EXTERNAL_REASON:
        return wlan_mlme::AssociateResultCodes::REFUSED_EXTERNAL_REASON;
    case ASSOC_RESULT_REFUSED_AP_OUT_OF_MEMORY:
        return wlan_mlme::AssociateResultCodes::REFUSED_AP_OUT_OF_MEMORY;
    case ASSOC_RESULT_REFUSED_BASIC_RATES_MISMATCH:
        return wlan_mlme::AssociateResultCodes::REFUSED_BASIC_RATES_MISMATCH;
    case ASSOC_RESULT_REJECTED_EMERGENCY_SERVICES_NOT_SUPPORTED:
        return wlan_mlme::AssociateResultCodes::REJECTED_EMERGENCY_SERVICES_NOT_SUPPORTED;
    case ASSOC_RESULT_REFUSED_TEMPORARILY:
        return wlan_mlme::AssociateResultCodes::REFUSED_TEMPORARILY;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::StartResultCodes ConvertStartResultCode(wlanif_start_result_codes code) {
    switch (code) {
    case START_RESULT_SUCCESS:
        return wlan_mlme::StartResultCodes::SUCCESS;
    case START_RESULT_BSS_ALREADY_STARTED_OR_JOINED:
        return wlan_mlme::StartResultCodes::BSS_ALREADY_STARTED_OR_JOINED;
    case START_RESULT_RESET_REQUIRED_BEFORE_START:
        return wlan_mlme::StartResultCodes::RESET_REQUIRED_BEFORE_START;
    case START_RESULT_NOT_SUPPORTED:
        return wlan_mlme::StartResultCodes::NOT_SUPPORTED;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::EapolResultCodes ConvertEapolResultCode(wlanif_eapol_result_codes code) {
    switch (code) {
    case EAPOL_RESULT_SUCCESS:
        return wlan_mlme::EapolResultCodes::SUCCESS;
    case EAPOL_RESULT_TRANSMISSION_FAILURE:
        return wlan_mlme::EapolResultCodes::TRANSMISSION_FAILURE;
    default:
        ZX_ASSERT(0);
    }
}

wlan_mlme::MacRole ConvertMacRole(mac_roles role) {
    switch(role) {
    case MAC_ROLE_CLIENT:
        return wlan_mlme::MacRole::CLIENT;
    case MAC_ROLE_AP:
        return wlan_mlme::MacRole::AP;
    default:
        ZX_ASSERT(0);
    }
}

void ConvertBandCapabilities(wlan_mlme::BandCapabilities* fidl_band,
                             wlanif_band_capabilities_t* band) {
    // basic_rates
    fidl_band->basic_rates.resize(band->num_basic_rates);
    fidl_band->basic_rates->assign(band->basic_rates, band->basic_rates + band->num_basic_rates);

    // base_frequency
    fidl_band->base_frequency = band->base_frequency;

    // channels
    fidl_band->channels.resize(band->num_channels);
    fidl_band->channels->assign(band->channels, band->channels + band->num_channels);
}

}  // namespace wlanif
