local BundlePlanner = require(script.Parent.Parent.sync.bundle-planner)
local Serializer = require(script.Parent.Parent.sync.serializer)
local TreeWalker = require(script.Parent.Parent.sync.tree-walker)
local DEFAULT_SYNC_CONFIG = table.freeze({bundleThreshold = 20, bundleGrouping = "className", smallBundleBehavior = "explode"})
function previewPull(root)
    local children = root:GetChildren()
    local plan = BundlePlanner.planChildren(children, DEFAULT_SYNC_CONFIG.bundleThreshold)
    local nodes = TreeWalker.walkInstanceTree(root)
    return {root = Serializer.serializeInstance(root), plan = plan, nodes = nodes}
end
return {DEFAULT_SYNC_CONFIG = DEFAULT_SYNC_CONFIG, previewPull = previewPull}
