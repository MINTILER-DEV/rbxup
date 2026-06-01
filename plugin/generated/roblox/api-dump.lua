local HttpService = game:GetService("HttpService")
local ApiDumpUrl = "https://raw.githubusercontent.com/MaximumADHD/Roblox-Client-Tracker/refs/heads/roblox/API-Dump.json"
local TrackedClasses = table.freeze({Object = true, Instance = true, PVInstance = true, Model = true, BasePart = true, FormFactorPart = true, TriangleMeshPart = true, PartOperation = true, Part = true, MeshPart = true, UnionOperation = true, Attachment = true, FaceInstance = true, Decal = true, Texture = true, Constraint = true, WeldConstraint = true, HingeConstraint = true, BallSocketConstraint = true, RodConstraint = true, RopeConstraint = true, SpringConstraint = true, LuaSourceContainer = true, BaseScript = true, Script = true, LocalScript = true, ModuleScript = true, GuiBase = true, GuiBase2d = true, GuiObject = true, GuiLabel = true, GuiButton = true, Frame = true, TextLabel = true, TextButton = true, ImageLabel = true, ImageButton = true, ValueBase = true, StringValue = true, IntValue = true, NumberValue = true, BoolValue = true, ObjectValue = true, Vector3Value = true, Color3Value = true, CFrameValue = true, Folder = true})
local cachedClassInfoByName = nil
local cachedLoadError = nil
function shouldIncludeProperty(member)
    local _cond0 = (member.MemberType ~= "Property")
    if _cond0 then
        return false
    end
    local serialization = member.Serialization
    local _cond1 = (serialization == nil)
    if _cond1 then
        return true
    end
    local _lhs2 = (serialization.CanLoad ~= false)
    return (_lhs2 or (serialization.CanSave ~= false))
end
function shouldTrackClass(className)
    return (TrackedClasses[className] == true)
end
function buildPropertySet(members)
    local properties = {}
    local _cond3 = (members == nil)
    if _cond3 then
        return table.freeze(properties)
    end
    for _, member in members do
        local _cond4 = shouldIncludeProperty(member)
        if _cond4 then
            local _idx5 = member.Name
            properties[_idx5] = true
        end
    end
    return table.freeze(properties)
end
function buildClassInfoByName(classes)
    local classInfoByName = {}
    for _, classInfo in classes do
        local _cond6 = shouldTrackClass(classInfo.Name)
        if _cond6 then
            local _idx7 = classInfo.Name
            classInfoByName[_idx7] = table.freeze({superclass = classInfo.Superclass, properties = buildPropertySet(classInfo.Members)})
        end
    end
    return table.freeze(classInfoByName)
end
function loadClassInfoByName()
    local _cond8 = (cachedClassInfoByName ~= nil)
    if _cond8 then
        return cachedClassInfoByName
    end
    local _cond9 = (cachedLoadError ~= nil)
    if _cond9 then
        return nil
    end
    local response = nil
    local okResponse = pcall(function()
    response = HttpService:GetAsync(ApiDumpUrl)
end)
    local _cond10 = (not okResponse)
    if _cond10 then
        cachedLoadError = response
        return nil
    end
    local decoded = nil
    local okDecode = pcall(function()
    decoded = HttpService:JSONDecode(response)
end)
    local _cond11 = (not okDecode)
    if _cond11 then
        cachedLoadError = decoded
        return nil
    end
    local _lhs12 = decoded.Classes
    cachedClassInfoByName = buildClassInfoByName((if _lhs12 ~= nil then _lhs12 else {}))
    return cachedClassInfoByName
end
function getClassInfo(className)
    local classInfoByName = loadClassInfoByName()
    local _cond13 = (classInfoByName == nil)
    if _cond13 then
        return nil
    end
    return classInfoByName[className]
end
function getLoadError()
    return cachedLoadError
end
function refresh()
    cachedClassInfoByName = nil
    cachedLoadError = nil
    return loadClassInfoByName()
end
return {ApiDumpUrl = ApiDumpUrl, getClassInfo = getClassInfo, getLoadError = getLoadError, refresh = refresh}
