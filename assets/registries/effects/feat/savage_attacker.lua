local FEAT_ID = "nat20_core::feat.savage_attacker"

---@type DamageRollResultHookFn
local function damage_roll_result_hook(engine_state, entity, damage_roll_result, action, resolution)
    -- Only when a weapon attack hits
    if not action.action_context:is_weapon_attack() then
        return
    end
    if not resolution:is_success() then
        return
    end

    -- Once per turn; always used on the first hit, since keeping the higher roll never hurts
    if not engine_state:use_once_per_turn(entity, FEAT_ID) then
        return
    end

    damage_roll_result:reroll_base_dice_keep_higher()
end

return {
    damage_roll_result_hook = damage_roll_result_hook,
}
