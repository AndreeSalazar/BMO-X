/*
 * ADead-BIB — Upper Layer Interface
 * FastOS C component header
 *
 * This will provide the C API for upper OS layers
 * once the Rust kernel base is stable.
 */

#ifndef ADEAD_BIB_H
#define ADEAD_BIB_H

/* Version */
#define ADEAD_VERSION_MAJOR 0
#define ADEAD_VERSION_MINOR 1
#define ADEAD_VERSION_PATCH 0

/* Status: not yet active */
#define ADEAD_STATUS_WAITING 0
#define ADEAD_STATUS_ACTIVE  1

/* Syscall numbers (future) */
#define SYSCALL_EXIT    0
#define SYSCALL_WRITE   1
#define SYSCALL_READ    2
#define SYSCALL_OPEN    3
#define SYSCALL_CLOSE   4
#define SYSCALL_MMAP    5

/* Types */
typedef unsigned long  uint64_t;
typedef unsigned int   uint32_t;
typedef unsigned short uint16_t;
typedef unsigned char  uint8_t;
typedef long           int64_t;
typedef int            int32_t;

/* ADead initialization — called by Rust kernel when ready */
int adead_init(void);

/* ADead status query */
int adead_get_status(void);

#endif /* ADEAD_BIB_H */
