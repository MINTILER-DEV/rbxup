local SyncCommand = {Pull = "pull", Push = "push", Diff = "diff", Doctor = "doctor"}
function makeMessage(command, payload)
    return {kind = "sync-message", version = 1, command = command, payload = (if payload ~= nil then payload else {})}
end
return {SyncCommand = table.freeze(SyncCommand), makeMessage = makeMessage}
