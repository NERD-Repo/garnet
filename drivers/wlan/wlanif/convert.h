// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#pragma once

#include <fuchsia/wlan/mlme/cpp/fidl.h>
#include <wlan/protocol/if-impl.h>

namespace wlanif {

namespace wlan_mlme = ::fuchsia::wlan::mlme;

wlanif_bss_types ConvertBSSType(wlan_mlme::BSSTypes bss_type);
wlanif_scan_types ConvertScanType(wlan_mlme::ScanTypes scan_type);
uint8_t ConvertCBW(wlan_mlme::CBW cbw);
void ConvertWlanChan(wlan_channel_t* wlanif_chan, wlan_mlme::WlanChan* fidl_chan);
void ConvertWlanChan(wlan_mlme::WlanChan* fidl_chan, wlan_channel_t* wlanif_chan);
void ConvertBSSDescription(wlanif_bss_description_t* wlanif_bss_desc,
                           wlan_mlme::BSSDescription* fidl_bss_desc);
void ConvertBSSDescription(wlan_mlme::BSSDescription* fidl_bss_desc,
                           wlanif_bss_description_t* wlanif_bss_desc);
wlanif_auth_types ConvertAuthType(wlan_mlme::AuthenticationTypes auth_type);
wlanif_deauth_reason_codes ConvertDeauthReasonCode(wlan_mlme::ReasonCode reason);
wlanif_key_types ConvertKeyType(wlan_mlme::KeyType key_type);
void ConvertSetKeyDescriptor(set_key_descriptor_t* key_desc,
                             wlan_mlme::SetKeyDescriptor* fidl_key_desc);
void ConvertDeleteKeyDescriptor(delete_key_descriptor_t* key_desc,
                                wlan_mlme::DeleteKeyDescriptor* fidl_key_desc);
wlan_mlme::BSSTypes ConvertBSSType(wlanif_bss_types bss_type);
wlan_mlme::CBW ConvertCBW(uint8_t cbw);
wlan_mlme::AuthenticationTypes ConvertAuthType(wlanif_auth_types auth_type);
wlan_mlme::ReasonCode ConvertDeauthReasonCode(wlanif_deauth_reason_codes reason);
wlan_mlme::ScanResultCodes ConvertScanResultCode(wlanif_scan_result_codes code);
wlan_mlme::JoinResultCodes ConvertJoinResultCode(wlanif_join_result_codes code);
wlan_mlme::AuthenticateResultCodes ConvertAuthResultCode(wlanif_auth_result_codes code);
wlan_mlme::AssociateResultCodes ConvertAssocResultCode(wlanif_assoc_result_codes code);
wlan_mlme::StartResultCodes ConvertStartResultCode(wlanif_start_result_codes code);
wlan_mlme::EapolResultCodes ConvertEapolResultCode(wlanif_eapol_result_codes code);
wlan_mlme::MacRole ConvertMacRole(mac_roles role);
void ConvertBandCapabilities(wlan_mlme::BandCapabilities* fidl_band,
                             wlanif_band_capabilities_t* band);

}  // namespace wlanif
