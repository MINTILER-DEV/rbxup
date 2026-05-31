type EncodedType = "Primitive" | "Vector2" | "Vector3" | "CFrame" | "Color3" | "UDim" | "UDim2" | "Enum" | "BrickColor" | "NumberRange" | "NumberSequence" | "ColorSequence" | "PhysicalProperties" | "Ref"
local EncodedType = table.freeze({
    Primitive = "Primitive" :: EncodedType,
    Vector2 = "Vector2" :: EncodedType,
    Vector3 = "Vector3" :: EncodedType,
    CFrame = "CFrame" :: EncodedType,
    Color3 = "Color3" :: EncodedType,
    UDim = "UDim" :: EncodedType,
    UDim2 = "UDim2" :: EncodedType,
    Enum = "Enum" :: EncodedType,
    BrickColor = "BrickColor" :: EncodedType,
    NumberRange = "NumberRange" :: EncodedType,
    NumberSequence = "NumberSequence" :: EncodedType,
    ColorSequence = "ColorSequence" :: EncodedType,
    PhysicalProperties = "PhysicalProperties" :: EncodedType,
    Ref = "Ref" :: EncodedType,
})
function encodeValue(value)
    local valueType = typeof(value)
    local _sw0 = valueType
    if _sw0 == "Vector2" then
        return {["type"] = EncodedType.Vector2, value = {value.X, value.Y}}
    elseif _sw0 == "Vector3" then
        return {["type"] = EncodedType.Vector3, value = {value.X, value.Y, value.Z}}
    elseif _sw0 == "CFrame" then
        local components = {value:GetComponents()}
        return {["type"] = EncodedType.CFrame, value = components}
    elseif _sw0 == "Color3" then
        return {["type"] = EncodedType.Color3, value = {value.R, value.G, value.B}}
    elseif _sw0 == "EnumItem" then
        return {["type"] = EncodedType.Enum, ["enum"] = tostring(value.EnumType), value = value.Name}
    else
        return {["type"] = EncodedType.Primitive, value = value}
    end
end
function decodeValue(encoded)
    local kind = encoded["type"]
    local value = encoded.value
    local enumName = encoded["enum"]
    local _sw1 = kind
    if _sw1 == EncodedType.Vector2 then
        return Vector2.new(value[1], value[2])
    elseif _sw1 == EncodedType.Vector3 then
        return Vector3.new(value[1], value[2], value[3])
    elseif _sw1 == EncodedType.CFrame then
        return CFrame.new(unpack(value))
    elseif _sw1 == EncodedType.Color3 then
        return Color3.new(value[1], value[2], value[3])
    elseif _sw1 == EncodedType.Enum then
        local enumType = Enum[enumName]
        return enumType[value]
    else
        return value
    end
end
return {EncodedType = EncodedType, encodeValue = encodeValue, decodeValue = decodeValue}
