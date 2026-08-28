(module
  (memory (export "memory") 1)
  (data (i32.const 2048) "thermostat:setpoint:accepted")
  (global $heap (mut i32) (i32.const 4096))
  (func (export "rivun_alloc") (param $len i32) (result i32)
    global.get $heap
    global.get $heap
    local.get $len
    i32.add
    global.set $heap)
  (func (export "rivun_dealloc") (param i32 i32))
  (func (export "rivun_execute")
    (param $action_ptr i32) (param $action_len i32)
    (param $payload_ptr i32) (param $payload_len i32)
    (result i64)
    i64.const 2048
    i64.const 32
    i64.shl
    i64.const 28
    i64.or))

