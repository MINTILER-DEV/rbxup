local ApiDump = require(script.Parent.Parent.roblox.api-dump)
local Defaults = require(script.Parent.Parent.roblox.defaults)
function getKnownProperties(className)
    local classInfo = ApiDump.getClassInfo(className)
    local _cond0 = (classInfo ~= nil)
    if _cond0 then
        return classInfo.properties
    end
    local _lhs1 = Defaults.MainProperties[className]
    return (if _lhs1 ~= nil then _lhs1 else {})
end
return {getKnownProperties = getKnownProperties}
