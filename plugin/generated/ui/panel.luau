function buildPanelState()
    return {
        title = "rbxup Sync",
        status = "idle",
        actions = {
            "Pull",
            "Push",
            "Diff",
            "Doctor",
        },
    }
end

return {
    buildPanelState = buildPanelState,
}
