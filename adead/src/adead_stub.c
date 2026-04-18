/*
 * ADead-BIB — Stub Implementation
 * Placeholder until Rust kernel base is complete.
 */

#include "../include/adead.h"

static int _adead_status = ADEAD_STATUS_WAITING;

int adead_init(void) {
    /* Will initialize upper layers when kernel is ready */
    _adead_status = ADEAD_STATUS_WAITING;
    return 0;
}

int adead_get_status(void) {
    return _adead_status;
}
