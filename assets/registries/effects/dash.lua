--- One more Speed's worth of movement for each Dash this turn
---@type SpeedHookFn
local function speed_hook(engine_state, entity, speed)
    speed:add_extra_speed("nat20_core::effect.dash")
end

return {
    speed_hook = speed_hook,
}
