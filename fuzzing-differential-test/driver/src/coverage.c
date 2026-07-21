// Copyright 2019 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Adapted from Fuzzilli's Targets/coverage.c for Rust's multiple LLVM modules.

#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

#define SHM_SIZE 0x200000U
#define MAX_EDGES ((SHM_SIZE - 4U) * 8U)

struct shmem_data {
    uint32_t num_edges;
    unsigned char edges[];
};

static struct shmem_data *shmem;
static uint32_t next_edge = 1U;

static void fail_coverage_initialization(const char *message) {
    fprintf(stderr, "Velum differential coverage initialization failed: %s\n", message);
    _exit(1);
}

static void initialize_shared_memory(void) {
    if (shmem != NULL) {
        return;
    }

    const char *shm_key = getenv("SHM_ID");
    if (shm_key == NULL) {
        shmem = calloc(1U, SHM_SIZE);
        if (shmem == NULL) {
            fail_coverage_initialization("allocation failed");
        }
        return;
    }

    const int fd = shm_open(shm_key, O_RDWR, S_IRUSR | S_IWUSR);
    if (fd < 0) {
        fail_coverage_initialization(strerror(errno));
    }

    void *mapping = mmap(NULL, SHM_SIZE, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (close(fd) != 0) {
        fail_coverage_initialization(strerror(errno));
    }
    if (mapping == MAP_FAILED) {
        fail_coverage_initialization(strerror(errno));
    }
    shmem = mapping;
}

void __sanitizer_cov_trace_pc_guard_init(uint32_t *start, uint32_t *stop) {
    if (start == stop || *start != 0U) {
        return;
    }

    initialize_shared_memory();
    for (uint32_t *guard = start; guard < stop; ++guard) {
        if (next_edge >= MAX_EDGES) {
            *guard = 0U;
            continue;
        }
        *guard = next_edge;
        ++next_edge;
    }
    shmem->num_edges = next_edge - 1U;
}

void __sanitizer_cov_trace_pc_guard(uint32_t *guard) {
    const uint32_t index = *guard;
    if (index == 0U || shmem == NULL) {
        return;
    }
    shmem->edges[index / 8U] |= (unsigned char)(1U << (index % 8U));
}

// Guards remain assigned so each execution can repopulate the bitmap after
// Fuzzilli clears it. This is slower than one-shot guards but avoids unsafe
// Rust FFI solely for resetting instrumentation state.
void __sanitizer_cov_reset_edgeguards(void) {}
