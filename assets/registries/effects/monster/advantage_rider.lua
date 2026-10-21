--- An extra 1d4 of the first damage type when the attack roll had Advantage
---@type DamageRollHookFn
local function damage_roll_hook(engine_state, entity, damage_roll, action, resolution)
    if not resolution:is_attack_roll() or not resolution:had_advantage() then
        return
    end

    local component = damage_roll.components[1]
    if not component then
        return
    end

    damage_roll:add_damage("1d4", component.damage_type, "nat20_core::effect.monster.advantage_rider")
end

return {
    damage_roll_hook = damage_roll_hook,
}
