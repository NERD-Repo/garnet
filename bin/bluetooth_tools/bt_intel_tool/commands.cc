// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "commands.h"

#include <endian.h>

#include <sys/mman.h>
#include <cstring>
#include <iostream>

#include "garnet/drivers/bluetooth/lib/common/manufacturer_names.h"
#include "garnet/drivers/bluetooth/lib/gap/advertising_data.h"
#include "garnet/drivers/bluetooth/lib/hci/advertising_report_parser.h"
#include "garnet/drivers/bluetooth/lib/hci/command_channel.h"
#include "garnet/drivers/bluetooth/lib/hci/control_packets.h"
#include "lib/fxl/strings/join_strings.h"
#include "lib/fxl/strings/string_number_conversions.h"
#include "lib/fxl/strings/string_printf.h"
#include "lib/fxl/time/time_delta.h"

#include "bt_intel.h"
#include "intel_firmware_loader.h"

using namespace bluetooth;

using std::placeholders::_1;
using std::placeholders::_2;

namespace bt_intel {
namespace {

class MfgModeEnabler {
 public:
  MfgModeEnabler(CommandChannel* channel)
      : channel_(channel), patch_reset_needed_(false) {
    auto packet = MakeMfgModePacket(true);
    channel_->SendCommand(packet->view());
  }

  ~MfgModeEnabler() {
    MfgDisableMode disable_mode = MfgDisableMode::kNoPatches;
    if (patch_reset_needed_)
      disable_mode = MfgDisableMode::kPatchesEnabled;

    auto packet = MakeMfgModePacket(false, disable_mode);
    channel_->SendCommand(packet->view());
  }

  void set_patch_reset(bool patch) { patch_reset_needed_ = patch; }

 private:
  CommandChannel* channel_;
  bool patch_reset_needed_;

  std::unique_ptr<hci::CommandPacket> MakeMfgModePacket(
      bool enable,
      MfgDisableMode disable_mode = MfgDisableMode::kNoPatches) {
    auto packet = hci::CommandPacket::New(
        kMfgModeChange, sizeof(IntelMfgModeChangeCommandParams));
    auto params = packet->mutable_view()
                      ->mutable_payload<IntelMfgModeChangeCommandParams>();
    params->enable = enable ? 1 : 0;
    params->disable_mode = disable_mode;
    return packet;
  }
};

void LogCommandComplete(hci::Status status) {
  std::cout << "  Command Complete - status: "
            << fxl::StringPrintf("0x%02x", status) << std::endl;
}

// Prints a byte in decimal and hex forms
std::string PrintByte(uint8_t byte) {
  return fxl::StringPrintf("%u (0x%02x)", byte, byte);
}

std::string EnableParamToString(hci::GenericEnableParam param) {
  return (param == hci::GenericEnableParam::kEnable) ? "enabled" : "disabled";
}

std::string FirmwareVariantToString(uint8_t fw_variant) {
  switch (fw_variant) {
    case 0x06:
      return "bootloader";
    case 0x23:
      return "firmware";
    default:
      break;
  }
  return "UNKNOWN";
}

bool HandleReadVersion(const CommandData* cmd_data,
                       const fxl::CommandLine& cmd_line,
                       const fxl::Closure& complete_cb) {
  if (cmd_line.positional_args().size()) {
    std::cout << "  Usage: read-version [--verbose]" << std::endl;
    return false;
  }

  auto cb = [cmd_line, complete_cb](const hci::EventPacket& event) {
    auto params = event.return_params<IntelVersionReturnParams>();
    LogCommandComplete(params->status);

    std::cout << fxl::StringPrintf(
        "  Firmware Summary: variant=%s - revision %u.%u build no: %u (week "
        "%u, year %u)",
        FirmwareVariantToString(params->fw_variant).c_str(),
        params->fw_revision >> 4, params->fw_revision & 0x0F,
        params->fw_build_num, params->fw_build_week,
        2000 + params->fw_build_year);
    std::cout << std::endl;

    if (cmd_line.HasOption("verbose")) {
      std::cout << "  Intel Read Version:" << std::endl;
      std::cout << "    Hardware Platform: " << PrintByte(params->hw_platform)
                << std::endl;
      std::cout << "    Hardware Variant:  " << PrintByte(params->hw_variant)
                << std::endl;
      std::cout << "    Hardware Revision: " << PrintByte(params->hw_revision)
                << std::endl;
      std::cout << "    Firmware Variant:  " << PrintByte(params->fw_variant)
                << std::endl;
      std::cout << "    Firmware Revision: " << PrintByte(params->fw_revision)
                << std::endl;
      std::cout << "    Firmware Build No: " << PrintByte(params->fw_build_num)
                << std::endl;
      std::cout << "    Firmware Build Week: "
                << PrintByte(params->fw_build_week) << std::endl;
      std::cout << "    Firmware Build Year: "
                << PrintByte(params->fw_build_year) << std::endl;
      std::cout << "    Firmware Patch No: " << PrintByte(params->fw_patch_num)
                << std::endl;
    }
  };

  //cmd_data->cmd_channel()->SetEventCallback(cb);

  auto packet = hci::CommandPacket::New(kReadVersion);
  std::cout << "  Sending HCI Vendor (Intel) Read Version" << std::endl;
  cmd_data->cmd_channel()->SendCommandSync(packet->view(), cb);

  complete_cb();
  return true;
}

bool HandleReadBootParams(const CommandData* cmd_data,
                          const fxl::CommandLine& cmd_line,
                          const fxl::Closure& complete_cb) {
  if (cmd_line.positional_args().size() || cmd_line.options().size()) {
    std::cout << "  Usage: read-boot-params" << std::endl;
    return false;
  }

  auto cb = [cmd_line, complete_cb, cmd_data](const hci::EventPacket& event) {
    auto params = event.return_params<IntelReadBootParamsReturnParams>();
    LogCommandComplete(params->status);

    std::cout << "  Intel Boot Parameters:" << std::endl;
    std::cout << "    Device Revision:  " << le16toh(params->dev_revid)
              << std::endl;
    std::cout << "    Secure Boot:      "
              << EnableParamToString(params->secure_boot) << std::endl;
    std::cout << "    OTP Lock:         "
              << EnableParamToString(params->otp_lock) << std::endl;
    std::cout << "    API Lock:         "
              << EnableParamToString(params->api_lock) << std::endl;
    std::cout << "    Debug Lock:       "
              << EnableParamToString(params->debug_lock) << std::endl;
    std::cout << "    Limited CCE:      "
              << EnableParamToString(params->limited_cce) << std::endl;
    std::cout << "    OTP BD_ADDR:      " << params->otp_bdaddr.ToString()
              << std::endl;
    std::cout << "    Minimum Firmware Build: "
              << fxl::StringPrintf("build no: %u (week %u, year %u)",
                                   params->min_fw_build_num,
                                   params->min_fw_build_week,
                                   2000 + params->min_fw_build_year)
              << std::endl;

    cmd_data->cmd_channel()->SetEventCallback(nullptr);
    complete_cb();
  };

  auto packet = hci::CommandPacket::New(kReadBootParams);
  cmd_data->cmd_channel()->SetEventCallback(cb);
  cmd_data->cmd_channel()->SendCommand(packet->view());
  std::cout << "  Sent HCI Vendor (Intel) Read Boot Params" << std::endl;

  return true;
}

bool HandleReset(const CommandData* cmd_data,
                 const fxl::CommandLine& cmd_line,
                 const fxl::Closure& complete_cb) {
  if (cmd_line.positional_args().size() || cmd_line.options().size()) {
    std::cout << "  Usage: reset" << std::endl;
    return false;
  }

  auto packet =
      hci::CommandPacket::New(kReset, sizeof(IntelResetCommandParams));
  auto params =
      packet->mutable_view()->mutable_payload<IntelResetCommandParams>();
  params->data[0] = 0x00;
  params->data[1] = 0x01;
  params->data[2] = 0x00;
  params->data[3] = 0x01;
  params->data[4] = 0x00;
  params->data[5] = 0x08;
  params->data[6] = 0x04;
  params->data[7] = 0x00;

  cmd_data->cmd_channel()->SendCommand(packet->view());
  std::cout << "  Sent HCI Vendor (Intel) Rese" << std::endl;

  // Once the reset command is sent, the hardware will shut down and we won't be
  // able to get a response back. Just exit the tool.

  complete_cb();
  return true;
}

bool HandleLoadBseq(const CommandData* cmd_data,
                    const fxl::CommandLine& cmd_line,
                    const fxl::Closure& complete_cb) {
  if (cmd_line.positional_args().size() != 1) {
    std::cout << "  Usage: load-bseq [--verbose] <filename>" << std::endl;
    return false;
  }

  std::string firmware_fn = cmd_line.positional_args().front();

  {
    MfgModeEnabler enable(cmd_data->cmd_channel());

    IntelFirmwareLoader loader(cmd_data->cmd_channel());

    IntelFirmwareLoader::LoadStatus patched = loader.LoadBseq(firmware_fn);

    if (patched == IntelFirmwareLoader::LoadStatus::kPatched) {
      enable.set_patch_reset(true);
    }
  }

  complete_cb();
  return true;
}

bool HandleLoadSecure(const CommandData* cmd_data,
                      const fxl::CommandLine& cmd_line,
                      const fxl::Closure& complete_cb) {
  if (cmd_line.positional_args().size() != 1) {
    std::cout << "  Usage: load-sfi [--verbose] <filename>" << std::endl;
    return false;
  }

  std::string firmware_fn = cmd_line.positional_args().front();

  IntelFirmwareLoader loader(cmd_data->cmd_channel());

  loader.LoadSfi(firmware_fn);

  complete_cb();
  return true;
}

}  // namespace

void RegisterCommands(const CommandData* data,
                      bluetooth::tools::CommandDispatcher* dispatcher) {
#define BIND(handler) \
  std::bind(&handler, data, std::placeholders::_1, std::placeholders::_2)

  dispatcher->RegisterHandler("read-version",
                              "Read hardware version information",
                              BIND(HandleReadVersion));
  dispatcher->RegisterHandler("read-boot-params",
                              "Read hardware boot parameters",
                              BIND(HandleReadBootParams));
  dispatcher->RegisterHandler("load-bseq", "Load bseq file onto device",
                              BIND(HandleLoadBseq));
  dispatcher->RegisterHandler("load-sfi", "Load Secure Firmware onto device",
                              BIND(HandleLoadSecure));

  dispatcher->RegisterHandler("reset", "Reset firmware", BIND(HandleReset));

#undef BIND
}

}  // namespace bt_intel
