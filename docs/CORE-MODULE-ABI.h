#pragma once
#include <stdint.h>
/* ABI v1 bootstrap surface. Never pass Rust structs, references, or allocators.
   Run on the original process main thread. Keep the DLL loaded until exit.
   Kind 1 = native CORE; kind 2 = control centre. */
#ifdef __cplusplus
extern "C" {
#endif
uint32_t dotisle_module_abi(void);
uint32_t dotisle_module_kind(void);
int32_t dotisle_module_run(void);
#ifdef __cplusplus
}
#endif

/* Internal services use UTF-8 JSON requests and explicit DLL-owned responses. */
#include <stddef.h>
typedef struct DotIsleBuffer { unsigned char *ptr; size_t len; } DotIsleBuffer;
#ifdef __cplusplus
extern "C" {
#endif
DotIsleBuffer dotisle_service_call(const unsigned char *request, size_t length);
void dotisle_service_free(DotIsleBuffer response);
#ifdef __cplusplus
}
#endif
