local HttpService = game:GetService("HttpService")
local ATTRIBUTE_NAME = "rbxup_id"
function isMissing(value)
    local _lhs0 = (value == nil)
    return (_lhs0 or (value == ""))
end
function ensureStableId(instance)
    local existing = instance:GetAttribute(ATTRIBUTE_NAME)
    local _cond1 = (not isMissing(existing))
    if _cond1 then
        return existing, false
    end
    local generated = HttpService:GenerateGUID(false)
    instance:SetAttribute(ATTRIBUTE_NAME, generated)
    return generated, true
end
function getStableId(instance)
    return instance:GetAttribute(ATTRIBUTE_NAME)
end
return {ATTRIBUTE_NAME = ATTRIBUTE_NAME, ensureStableId = ensureStableId, getStableId = getStableId}
