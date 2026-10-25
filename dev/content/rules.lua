-- Edit and save: the host activates a valid replacement without resetting players.
-- Rust owns identity, persistence, authority and the maximum movement distance.
local distance = 12
return {
    schema_version = 1,
    world_name = "Argon development world",
    step = function(x, y, dx, dy)
        return x + dx * distance, y + dy * distance
    end,
}
