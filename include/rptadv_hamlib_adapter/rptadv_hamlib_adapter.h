/** @file rptadv_hamlib_adapter.h
 * Versioned, thread-owned Hamlib dependency adapter. No native Hamlib types
 * cross this boundary. All operations belong to the rig's non-audio owner.
 */
#ifndef RPTADV_HAMLIB_ADAPTER_H
#define RPTADV_HAMLIB_ADAPTER_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

/** Borrowed UTF-8 bytes. Inputs are copied by open; outputs live with the DSO.
 */
typedef struct rptadv_hamlib_string {
  const char *data; /**< Bytes, not necessarily NUL terminated. */
  size_t size;      /**< Number of bytes. */
} rptadv_hamlib_string;

/** Named Hamlib transport option, never a raw CAT command. */
typedef struct rptadv_hamlib_option {
  rptadv_hamlib_string name;  /**< Installed transport option name. */
  rptadv_hamlib_string value; /**< Value; errors never echo it. */
} rptadv_hamlib_option;

/** Complete initialization contract. One handle represents one configured rig.
 */
typedef struct rptadv_hamlib_config {
  size_t struct_size;                  /**< Exact record size. */
  uint32_t abi_version;                /**< One. */
  uint32_t reserved;                   /**< Zero. */
  rptadv_hamlib_string model;          /**< Canonical RIG_MODEL_* name. */
  const rptadv_hamlib_option *options; /**< Borrowed option array. */
  size_t option_count;                 /**< Array length. */
  uint32_t timeout_ms; /**< Positive per-exchange timeout, at most 10000 ms. */
  uint32_t retries;    /**< Transaction retries, at most 10. */
  uint32_t
      timeout_retries; /**< Additional read-timeout retries, at most 100. */
  uint32_t reserved2;  /**< Zero. */
} rptadv_hamlib_config;

/** Stable primary setting selectors. Catalog entries also expose levels and
 * functions. */
enum rptadv_hamlib_selectors {
  RPTADV_HAMLIB_RECEIVE_HZ = 1,  /**< Receive base frequency. */
  RPTADV_HAMLIB_TRANSMIT_HZ = 2, /**< Absolute split transmit base frequency. */
  RPTADV_HAMLIB_MODE =
      3, /**< Shared mode selector, FM=1, AM=2, USB=3, LSB=4. */
  RPTADV_HAMLIB_BANDWIDTH_HZ = 4, /**< Receive passband, zero means normal. */
  RPTADV_HAMLIB_RECEIVE_VFO =
      5, /**< Current=0, A=1, B=2, main=3, sub=4, memory=5. */
  RPTADV_HAMLIB_TRANSMIT_VFO = 6,  /**< Same VFO selectors. */
  RPTADV_HAMLIB_SPLIT = 7,         /**< Split enable. */
  RPTADV_HAMLIB_OFFSET_HZ = 8,     /**< Signed repeater shift. */
  RPTADV_HAMLIB_RIT_HZ = 9,        /**< Receive incremental tuning. */
  RPTADV_HAMLIB_XIT_HZ = 10,       /**< Transmit incremental tuning. */
  RPTADV_HAMLIB_POWER_MW = 11,     /**< Established RF power. */
  RPTADV_HAMLIB_ENCODE_CTCSS = 12, /**< Tenths of hertz, zero disables. */
  RPTADV_HAMLIB_DECODE_CTCSS = 13, /**< Tenths of hertz, zero disables. */
  RPTADV_HAMLIB_ENCODE_DCS =
      14, /**< Actual nine-bit code, normal polarity only. */
  RPTADV_HAMLIB_DECODE_DCS =
      15, /**< Actual nine-bit code, normal polarity only. */
  RPTADV_HAMLIB_ENCODE_ENABLED = 16, /**< CTCSS encoder enable. */
  RPTADV_HAMLIB_DECODE_ENABLED = 17, /**< CTCSS decoder enable. */
  RPTADV_HAMLIB_PTT = 20, /**< Explicitly selected PTT provider only. */
  RPTADV_HAMLIB_CARRIER_DETECT = 21 /**< Rig carrier observation. */
};

/** One copied capability. Missing getters never imply successful readback. */
typedef struct rptadv_hamlib_setting {
  size_t struct_size;        /**< Exact record size supplied by caller. */
  uint32_t selector;         /**< Stable adapter-neutral setting. */
  uint32_t access;           /**< Bit 0 readable; bit 1 writable. */
  rptadv_hamlib_string name; /**< Stable ASCII name, localized by caller. */
  uint32_t decimal_places;   /**< Value is integer / 10^decimal_places. */
  uint32_t reserved;         /**< Zero. */
  int64_t minimum;           /**< Inclusive lower bound. */
  int64_t maximum;           /**< Inclusive upper bound. */
  size_t choice_count; /**< Nonzero restricts accepted values to choices. */
} rptadv_hamlib_setting;

/** One copied runtime hardware coverage interval. */
typedef struct rptadv_hamlib_range {
  size_t struct_size;  /**< Exact caller-supplied record size. */
  uint64_t minimum_hz; /**< Inclusive receive/transmit base frequency. */
  uint64_t maximum_hz; /**< Inclusive upper frequency. */
  uint64_t modes;      /**< One shifted by each shared mode selector; bit zero
                          reserved. */
  int32_t minimum_power_mw; /**< Native bound; negative means unspecified. */
  int32_t maximum_power_mw; /**< Native bound; negative means unspecified. */
} rptadv_hamlib_range;

/** ABI descriptor. Zero is success; one ends enumeration; negative is failure.
 * -1001 unknown model, -1002 invalid option, -1003 invalid contract/value,
 * -1004 wrong owner thread. Other negative values retain Hamlib's error code.
 * All calls on an opened handle, including destroy, use its creating thread.
 */
typedef struct rptadv_hamlib_api {
  size_t struct_size;          /**< Complete descriptor size. */
  uint32_t abi_version;        /**< One. */
  uint32_t reserved;           /**< Zero. */
  const char *name;            /**< NUL-terminated provider name. */
  size_t (*model_count)(void); /**< Number of installed symbolic model names. */
  int32_t (*model_name)(
      size_t index, rptadv_hamlib_string *out); /**< Copy one borrowed name. */
  int32_t (*open)(const rptadv_hamlib_config *config,
                  void **out); /**< Configure and open. */
  int32_t (*close)(
      void *rig); /**< Unkey if owned, close, preserve first failure. */
  void (*destroy)(void *rig); /**< Release one handle; null is allowed. */
  int32_t (*setting)(
      void *rig, size_t index,
      rptadv_hamlib_setting *out); /**< Enumerate actual capabilities. */
  int32_t (*choice)(void *rig, uint32_t selector, size_t index,
                    int64_t *out); /**< Enumerate discrete choices. */
  int32_t (*get)(void *rig, uint32_t selector,
                 int64_t *out); /**< Read actual state. */
  int32_t (*set)(void *rig, uint32_t selector,
                 int64_t value); /**< Apply one admitted setting. */
  int32_t (*transport_budget_ms)(
      void *rig,
      uint64_t *out); /**< Nominal exchange budget; not cancellation. */
  int32_t (*range)(
      void *rig, uint32_t transmit, size_t index,
      rptadv_hamlib_range *out); /**< Enumerate negotiated coverage. */
  int32_t (*set_mode_width)(void *rig, int64_t mode,
                            uint32_t width_hz); /**< Complete mode/passband
                                                   write; no getter required. */
  int32_t (*set_split_vfos)(void *rig, int64_t receive_vfo, uint32_t enabled,
                            int64_t transmit_vfo); /**< Complete split routing;
                                                      enabled is zero or one. */
  int32_t (*set_power_mw)(
      void *rig, uint32_t power_mw, uint64_t transmit_hz,
      int64_t mode); /**< Absolute power using effective TX context. */
} rptadv_hamlib_api;

/** Borrow immutable descriptor while the DSO remains loaded. */
const rptadv_hamlib_api *rptadv_hamlib_api_v1(void);
#ifdef __cplusplus
}
#endif
#endif
