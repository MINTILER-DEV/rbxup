local StableIds = require(script.Parent.Parent.roblox.stable-ids)
local TypeCodec = require(script.Parent.Parent.roblox.type-codec)

function serializeInstance(instance)
    local id = StableIds.getStableId(instance)
    local encodedName = TypeCodec.encodeValue(instance.Name)

    return {
        format = "xup",
        version = 1,
        id = id,
        className = instance.ClassName,
        name = instance.Name,
        properties = {
            Name = encodedName,
        },
        attributes = {},
        tags = {},
    }
end

return {
    serializeInstance = serializeInstance,
}
