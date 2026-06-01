local TypeCodec = require(script.Parent.Parent.roblox.type-codec)

function applyProperties(instance, properties)
    for propertyName, encoded in properties do
        local ok, decoded = pcall(TypeCodec.decodeValue, encoded)
        if ok then
            pcall(function()
                instance[propertyName] = decoded
            end)
        end
    end
end

return {
    applyProperties = applyProperties,
}
