-- Settlement proposals only. Rust and the construction authority keep all checks.
-- Entrances face -world-y. The empty southern forecourt connects the first bays.
return {
    schema_version = 1,
    offsets = function(species, founded)
        if not founded then return {{0, 0}} end
        if species == 'organic' then
            -- Unequal lobes follow a branching settlement edge.
            return {{-19,-2},{22,3},{-37,-1},{40,2},{-55,0},{57,1},
                    {-34,22},{36,24},{-46,40},{46,42},{-4,43},{15,52}}
        elseif species == 'cyborg' then
            -- Paired terraces around a central access spine.
            return {{-20,0},{20,0},{-40,0},{40,0},{-60,0},{60,0},
                    {-24,24},{24,24},{-32,48},{32,48},{-48,26},{48,26}}
        elseif species == 'robot' then
            -- Repeatable manufacturing bays with a broad service forecourt.
            return {{-20,0},{20,0},{-40,0},{40,0},{-20,24},{20,24},
                    {-40,24},{40,24},{-20,-24},{20,-24},{-40,-24},{40,-24}}
        end
    end,
}
