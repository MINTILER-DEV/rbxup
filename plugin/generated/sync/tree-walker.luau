local StableIds = require(script.Parent.Parent.roblox.stable-ids)

function walkInstanceTree(root)
    local results = {}

    local function visit(node)
        local id = StableIds.getStableId(node)
        table.insert(results, {
            id = id,
            className = node.ClassName,
            name = node.Name,
            childCount = #node:GetChildren(),
        })

        for _, child in node:GetChildren() do
            visit(child)
        end
    end

    visit(root)
    return results
end

return {
    walkInstanceTree = walkInstanceTree,
}
