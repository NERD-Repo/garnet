// Copyright 2018 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include <fcntl.h>
#include <stdio.h>
#include <unistd.h>

#include "garnet/lib/wlan/fidl2/fidl2.fidl.cc.h"
#include <wlan/protocol/ioctl.h>

int main(int argc, const char** argv) {
    int fd = open("/dev/misc/test/wlan/wlanphy-test", O_RDWR);
    if (fd < 0) {
        fprintf(stderr, "could not open device: %d\n", fd);
        return -1;
    }

    zx::channel local, remote;
    zx_status_t status = zx::channel::create(0u, &local, &remote);
    if (status != ZX_OK) {
        fprintf(stderr, "could not create channel: %d\n", status);
        close(fd);
        return -1;
    }

    wlan::PhySyncPtr phy;
    phy.Bind(std::move(local));

    zx_handle_t remote_hnd = remote.release();
    status = ioctl_wlanphy_connect(fd, &remote_hnd);
    if (status < 0) {
        fprintf(stderr, "could not open phy: %d\n", status);
        close(fd);
        return -1;
    }

    wlan::QueryResponse resp;
    status = phy->Query(&resp);
    if (status < 0) {
        fprintf(stderr, "error in query: %d\n", status);
        close(fd);
        return -1;
    }

    fprintf(stderr, "SUCCESS\n");
    close(fd);
    return 0;
}
