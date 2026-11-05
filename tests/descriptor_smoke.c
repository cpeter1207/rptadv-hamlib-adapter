/** @file descriptor_smoke.c Installed dynamic Hamlib ABI lifecycle test. */
#include <assert.h>
#include <rptadv_hamlib_adapter/rptadv_hamlib_adapter.h>
#include <string.h>

int main(void) {
  const rptadv_hamlib_api *api = rptadv_hamlib_api_v1();
  assert(api && api->abi_version == 1 && api->struct_size == sizeof(*api));
  assert(api->model_count() > 100);
  rptadv_hamlib_string model = {0};
  assert(api->model_name(0, &model) == 0 && model.size > 0);
  rptadv_hamlib_config config = {.struct_size = sizeof(config),
                                 .abi_version = 1,
                                 .model = {"RIG_MODEL_DUMMY", 15},
                                 .timeout_ms = 100,
                                 .retries = 1};
  void *rig = NULL;
  assert(api->open(&config, &rig) == 0 && rig);
  assert(api->set(rig, RPTADV_HAMLIB_RECEIVE_HZ, 146520000) == 0);
  int64_t value = 0;
  assert(api->get(rig, RPTADV_HAMLIB_RECEIVE_HZ, &value) == 0 &&
         value == 146520000);
  assert(api->set(rig, RPTADV_HAMLIB_MODE, 3) == 0);
  assert(api->set(rig, RPTADV_HAMLIB_BANDWIDTH_HZ, 2400) == 0);
  assert(api->set_mode_width(rig, 3, 2400) == 0);
  assert(api->set_split_vfos(rig, 1, 1, 2) == 0);
  assert(api->set_split_vfos(rig, 1, 2, 2) < 0);
  assert(api->set_power_mw(rig, 5000, 146520000, 1) == 0);
  assert(api->set_split_mode_width(rig, 3, 2400) == 0);
  assert(api->get(rig, RPTADV_HAMLIB_SPLIT_MODE, &value) == 0 && value == 3);
  uint32_t power_mw = 0;
  assert(api->get_power_mw(rig, 146520000, 3, &power_mw) == 0 && power_mw > 0);
  assert(api->get_power_mw(rig, 146520000, 3, NULL) < 0);
  assert(api->get(rig, RPTADV_HAMLIB_MODE, &value) == 0 && value == 3);
  assert(api->set(rig, 0xffffffffU, 1) < 0);
  rptadv_hamlib_setting entry = {.struct_size = sizeof(entry)};
  assert(api->setting(rig, 0, &entry) == 0 && entry.selector > 0);
  rptadv_hamlib_range coverage = {.struct_size = sizeof(coverage)};
  assert(api->range(rig, 0, 0, &coverage) == 0 &&
         coverage.maximum_hz > coverage.minimum_hz);
  assert(api->close(rig) == 0);
  assert(api->get(rig, RPTADV_HAMLIB_RECEIVE_HZ, &value) < 0);
  api->destroy(rig);
  config.struct_size--;
  rig = NULL;
  assert(api->open(&config, &rig) < 0 && !rig);
  assert(api->open(NULL, &rig) < 0);
  assert(api->open(&config, NULL) < 0);
  assert(api->get(NULL, RPTADV_HAMLIB_MODE, &value) < 0);
  api->destroy(NULL);
  return 0;
}
