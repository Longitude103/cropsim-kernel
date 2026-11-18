/* Time the CropSim kernel on the benchmark cases, and save every output.
 *
 *   bench CASES [OUT|-] [REPEATS] [THREADS]
 *
 * Runs every case REPEATS times (default 20) on THREADS threads (default 1)
 * and prints the best single-thread time per run, or the throughput with
 * threads. With OUT, the first pass writes each run's state_out and result
 * structs to OUT, so two builds can be compared byte for byte (cmp), or
 * field by field with compare.py.
 */
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "cropsim_kernel.h"

static int n_cases, n_threads, repeats;
static csk_params_t params;
static char *cases;
static size_t case_size;

static double now(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec * 1e-9;
}

static void run_case(int i, csk_state_t *state_out, csk_result_t *res) {
  char *c = cases + case_size * i;
  const csk_sim_t *sim = (void *)c;          c += sizeof(csk_sim_t);
  const csk_soil_t *soil = (void *)c;        c += sizeof(csk_soil_t);
  const csk_weather_t *wx = (void *)c;       c += sizeof(csk_weather_t);
  const csk_state_t *state_in = (void *)c;   c += sizeof(csk_state_t);
  const csk_options_t *opt = (void *)c;
  csk_run_year(&params, sim, soil, wx, state_in, opt, state_out, res);
}

static void *worker(void *arg) {
  long id = (long)arg;
  csk_state_t st;
  csk_result_t *res = malloc(sizeof *res);
  for (int r = 0; r < repeats; r++)
    for (int i = (int)id; i < n_cases; i += n_threads) run_case(i, &st, res);
  free(res);
  return NULL;
}

int main(int argc, char **argv) {
  if (argc < 2) {
    fprintf(stderr, "usage: bench CASES [OUT|-] [REPEATS] [THREADS]\n");
    return 2;
  }
  FILE *f = fopen(argv[1], "rb");
  if (!f || fread(&n_cases, sizeof n_cases, 1, f) != 1 ||
      fread(&params, sizeof params, 1, f) != 1) {
    fprintf(stderr, "cannot read %s\n", argv[1]);
    return 1;
  }
  case_size = sizeof(csk_sim_t) + sizeof(csk_soil_t) + sizeof(csk_weather_t) +
              sizeof(csk_state_t) + sizeof(csk_options_t);
  cases = malloc(case_size * n_cases);
  if (fread(cases, case_size, n_cases, f) != (size_t)n_cases) {
    fprintf(stderr, "%s is short\n", argv[1]);
    return 1;
  }
  fclose(f);
  repeats = argc > 3 ? atoi(argv[3]) : 20;
  n_threads = argc > 4 ? atoi(argv[4]) : 1;

  csk_state_t st;
  csk_result_t *res = malloc(sizeof *res);
  if (argc > 2 && strcmp(argv[2], "-") != 0) {
    FILE *o = fopen(argv[2], "wb");
    for (int i = 0; i < n_cases; i++) {
      run_case(i, &st, res);
      fwrite(&st, sizeof st, 1, o);
      fwrite(res, sizeof *res, 1, o);
    }
    fclose(o);
  }

  if (n_threads <= 1) {
    double best = 1e30;
    for (int r = 0; r < repeats; r++) {
      double t0 = now();
      for (int i = 0; i < n_cases; i++) run_case(i, &st, res);
      double dt = now() - t0;
      if (dt < best) best = dt;
    }
    printf("%d runs, best of %d: %.1f ms, %.1f us per run (%.0f runs/s)\n", n_cases,
           repeats, best * 1e3, best * 1e6 / n_cases, n_cases / best);
  } else {
    pthread_t th[256];
    if (n_threads > 256) n_threads = 256;
    double t0 = now();
    for (long i = 0; i < n_threads; i++) pthread_create(&th[i], NULL, worker, (void *)i);
    for (int i = 0; i < n_threads; i++) pthread_join(th[i], NULL);
    double dt = now() - t0;
    printf("%d threads: %.0f runs/s\n", n_threads, (double)n_cases * repeats / dt);
  }
  free(res);
  free(cases);
  return 0;
}
