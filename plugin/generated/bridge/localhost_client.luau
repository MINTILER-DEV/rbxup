local Messages = require(script.Parent.Parent.bridge.messages)
local DEFAULT_HOST = "http://127.0.0.1:49321"
function buildRequest(command, payload)
    return {url = DEFAULT_HOST, message = Messages.makeMessage(command, payload)}
end
function ping(payload)
    return buildRequest(Messages.SyncCommand.Doctor, payload)
end
return {DEFAULT_HOST = DEFAULT_HOST, buildRequest = buildRequest, ping = ping}
