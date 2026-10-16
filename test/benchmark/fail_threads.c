#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <stdatomic.h>

/* Permit one worker, then simulate resource exhaustion. */
int pthread_create(pthread_t *thread, const pthread_attr_t *attributes,
                   void *(*function)(void *), void *argument)
{
    static atomic_int calls = 0;
    if (atomic_fetch_add(&calls, 1) >= 1) return EAGAIN;
    int (*create)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *) =
        dlsym(RTLD_NEXT, "pthread_create");
    if (!create) return EAGAIN;
    return create(thread, attributes, function, argument);
}
