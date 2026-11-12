#pragma once

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    TG_PROTO_BLE = 0,
    TG_PROTO_ZIGBEE = 1,
    TG_PROTO_SYSTEM = 2,
} tg_protocol_t;

typedef enum {
    TG_VAL_INT = 0,
    TG_VAL_FLOAT = 1,
    TG_VAL_BOOL = 2,
    TG_VAL_STRING = 3,
} tg_val_type_t;

typedef union {
    int64_t i_val;
    double f_val;
    bool b_val;
    const char *s_val;
} tg_val_union_t;

typedef struct {
    tg_val_type_t type;
    tg_val_union_t val;
} tg_value_t;

/**
 * @brief Standardized IoT event from BLE, Zigbee or System
 */
typedef struct {
    const char *device_id;
    tg_protocol_t protocol;
    uint8_t endpoint;
    const char *attribute;
    tg_value_t value;
    uint64_t timestamp_ms;
} tg_event_t;

/**
 * @brief Standardized Action produced by Rule Engine
 */
typedef struct {
    const char *target_device;
    tg_protocol_t protocol;
    uint8_t endpoint;
    const char *command;
    tg_value_t param;
} tg_action_t;

/**
 * @brief Callback invoked by the rule engine when an action is triggered.
 *        Executes in C context to dispatch to Zigbee, BLE, or MQTT queue.
 */
typedef void (*tg_action_callback_t)(const tg_action_t *action, void *user_data);

/**
 * @brief Initialize the Rust rule engine
 * @return 0 on success, negative error code on failure
 */
int32_t tg_rule_engine_init(void);

/**
 * @brief Dispatch an event into the rule engine for TCA evaluation
 * @param event Incoming sensor/system event
 * @param cb Callback called for each triggered action
 * @param user_data User context pointer passed to callback
 * @return Number of actions triggered, or negative on error
 */
int32_t tg_rule_engine_dispatch(const tg_event_t *event, tg_action_callback_t cb, void *user_data);

/**
 * @brief Returns the version / status banner of the Rust rule engine
 */
const char *tg_rule_engine_version(void);

#ifdef __cplusplus
}
#endif
