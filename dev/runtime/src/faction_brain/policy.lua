-- Policy receives only feasible candidate IDs, roles and exact sortable scores.
-- Rust owns visibility, budgets, validation and all eventual world effects.
return {
    schema_version = 1,
    choose = function(options)
        local best = nil
        for _, option in ipairs(options) do
            if best == nil
                or option.score_key > best.score_key
                or (option.score_key == best.score_key and option.id < best.id) then
                best = option
            end
        end
        return best and best.id or nil
    end,
}
