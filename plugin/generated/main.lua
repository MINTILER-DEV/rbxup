local Panel = require(script.Parent.ui.panel)
local LocalhostClient = require(script.Parent.bridge.localhost-client)
local SyncEngine = require(script.Parent.sync.sync-engine)
local PluginState = table.freeze({name = "rbxup Sync", version = 1})
function createPluginState()
    local panel = Panel.buildPanelState()
    local bridge = LocalhostClient.ping({source = "plugin"})
    return {plugin = PluginState, panel = panel, bridge = bridge, syncConfig = SyncEngine.DEFAULT_SYNC_CONFIG}
end
return {createPluginState = createPluginState}
