local FRENZY = "nat20_core::effect.barbarian.berserker.frenzy"

--- The Rage Damage bonus for a Barbarian level (SRD 5.2.1 Barbarian Features table)
---@param level integer
---@return integer
local function rage_damage(level)
    if level >= 16 then
        return 4
    elseif level >= 9 then
        return 3
    end
    return 2
end

---@type DamageRollHookFn
local function damage_roll_hook(engine_state, entity, damage_roll, action, resolution)
    if not resolution:is_attack_roll() or not resolution:is_success() then
        return
    end
    if not engine_state:has_effect(entity, "nat20_core::effect.barbarian.rage")
        or not engine_state:has_effect(entity, "nat20_core::effect.barbarian.reckless_attack_advantage")
        or not engine_state:is_turn_of(entity) then
        return
    end

    for _, component in ipairs(damage_roll.components) do
        if component.damage:get_modifier("strength") then
            -- Only the first target hit on the turn
            if not engine_state:use_once_per_turn(entity, FRENZY) then
                return
            end
            local level = engine_state:class_level(entity, "nat20_core::class.barbarian")
            damage_roll:add_damage(rage_damage(level) .. "d6", component.damage_type, FRENZY)
            return
        end
    end
end

return {
    damage_roll_hook = damage_roll_hook,
}
