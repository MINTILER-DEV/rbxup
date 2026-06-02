local ApiDump = require(script.Parent.Parent.roblox.api_dump)
local Defaults = require(script.Parent.Parent.roblox.defaults)
function appendMissingProperties(target, seen, properties)
    local _cond0 = (properties == nil)
    if _cond0 then
        return
    end
    for propertyName, enabled in properties do
        local _lhs1 = (enabled == true)
        local _cond2 = (_lhs1 and (seen[propertyName] ~= true))
        if _cond2 then
            seen[propertyName] = true
            local _idx3 = ((#target) + 1)
            target[_idx3] = propertyName
        end
    end
end
function getKnownProperties(className)
    local merged = {}
    local seenProperties = {}
    local visitedClasses = {}
    local currentClassName = className
    while true do
        local _lhs4 = (currentClassName ~= nil)
        local _cond5 = (_lhs4 and (visitedClasses[currentClassName] ~= true))
        local _while6 = _cond5
        if not _while6 then
            break
        end
        visitedClasses[currentClassName] = true
        local classInfo = ApiDump.getClassInfo(currentClassName)
        local _cond7 = (classInfo == nil)
        if _cond7 then
            break
        end
        appendMissingProperties(merged, seenProperties, classInfo.properties)
        local _cond8 = (classInfo.superclass == "<<<ROOT>>>")
        if _cond8 then
            break
        end
        currentClassName = classInfo.superclass
    end
    local _cond9 = ((#merged) > 0)
    if _cond9 then
        table.sort(merged)
        return merged
    end
    local _lhs10 = Defaults.MainProperties[className]
    return (if _lhs10 ~= nil then _lhs10 else {})
end
return {getKnownProperties = getKnownProperties}
