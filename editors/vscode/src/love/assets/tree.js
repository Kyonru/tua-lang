"use strict";

const vscode = require("vscode");
const { COMMANDS } = require("../../constants");

const CATEGORY_ICONS = {
  image: "file-media",
  audio: "unmute",
  audioVideo: "device-camera-video",
  font: "symbol-text",
  shader: "symbol-color",
  video: "device-camera-video",
};

class AssetTreeDataProvider {
  constructor() {
    this.assets = [];
    this.changed = new vscode.EventEmitter();
    this.onDidChangeTreeData = this.changed.event;
  }

  dispose() {
    this.changed.dispose();
  }

  update(assets) {
    this.assets = Array.isArray(assets) ? assets : [];
    this.changed.fire(undefined);
  }

  getTreeItem(element) {
    switch (element.kind) {
      case "category":
        return categoryTreeItem(element);
      case "usage":
        return usageTreeItem(element);
      case "asset":
        return assetTreeItem(element);
      case "reference":
        return referenceTreeItem(element);
      default:
        return new vscode.TreeItem("Unknown asset item");
    }
  }

  getChildren(element) {
    if (!element) {
      return categoryNodes(this.assets);
    }

    if (element.kind === "category") {
      return usageNodes(element.assets);
    }

    if (element.kind === "usage") {
      return element.assets.map((asset) => ({ kind: "asset", asset }));
    }

    if (element.kind === "asset") {
      return (element.asset.references || []).map((reference) => ({
        kind: "reference",
        reference,
      }));
    }

    return [];
  }
}

function categoryNodes(assets) {
  const categories = new Map();

  for (const asset of assets) {
    const category = asset.category || "other";
    const existing = categories.get(category);

    if (existing) {
      existing.assets.push(asset);
    } else {
      categories.set(category, {
        kind: "category",
        category,
        label: asset.categoryLabel || category,
        assets: [asset],
      });
    }
  }

  return [...categories.values()];
}

function usageNodes(assets) {
  const used = assets.filter((asset) => assetUsageCount(asset) > 0);
  const unused = assets.filter((asset) => assetUsageCount(asset) === 0);
  const groups = [];

  if (used.length > 0) {
    groups.push({ kind: "usage", usage: "used", label: "Used", assets: used });
  }

  if (unused.length > 0) {
    groups.push({ kind: "usage", usage: "unused", label: "Unused", assets: unused });
  }

  return groups;
}

function categoryTreeItem(node) {
  const item = new vscode.TreeItem(
    `${node.label} (${node.assets.length})`,
    vscode.TreeItemCollapsibleState.Collapsed,
  );

  item.contextValue = "tuaAssetCategory";
  item.iconPath = new vscode.ThemeIcon(CATEGORY_ICONS[node.category] || "files");
  return item;
}

function usageTreeItem(node) {
  const item = new vscode.TreeItem(
    `${node.label} (${node.assets.length})`,
    vscode.TreeItemCollapsibleState.Collapsed,
  );

  item.contextValue = `tuaAssetUsage.${node.usage}`;
  item.iconPath = new vscode.ThemeIcon(node.usage === "used" ? "references" : "circle-outline");
  return item;
}

function assetTreeItem(node) {
  const { asset } = node;
  const count = assetUsageCount(asset);
  const item = new vscode.TreeItem(
    asset.path,
    count > 0 ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None,
  );

  item.command = {
    command: COMMANDS.openAsset,
    title: "Open Asset",
    arguments: [node],
  };
  item.contextValue = count > 0 ? "tuaAsset.used" : "tuaAsset.unused";
  item.description = assetDescription(asset, count);
  item.resourceUri = vscode.Uri.parse(asset.uri);
  item.tooltip = assetTooltip(asset, count);
  return item;
}

function referenceTreeItem(node) {
  const { reference } = node;
  const uri = vscode.Uri.parse(reference.uri);
  const item = new vscode.TreeItem(
    vscode.workspace.asRelativePath(uri),
    vscode.TreeItemCollapsibleState.None,
  );

  item.command = {
    command: COMMANDS.openAssetReference,
    title: "Open Asset Reference",
    arguments: [node],
  };
  item.contextValue = "tuaAssetReference";
  item.description = `line ${reference.range.start.line + 1}, ${reference.loaderKind}`;
  item.iconPath = new vscode.ThemeIcon("go-to-file");
  item.tooltip = `${vscode.workspace.asRelativePath(uri)}:${reference.range.start.line + 1}`;
  return item;
}

function assetDescription(asset, count) {
  const parts = [];

  if (asset.dimensions) {
    parts.push(`${asset.dimensions.width}x${asset.dimensions.height}`);
  }

  parts.push(count === 1 ? "1 reference" : `${count} references`);
  return parts.join("  ");
}

function assetTooltip(asset, count) {
  const lines = [asset.path, `${count} static ${count === 1 ? "reference" : "references"}`];

  if (asset.dimensions) {
    lines.push(`${asset.dimensions.width}x${asset.dimensions.height}`);
  }

  if (count === 0) {
    lines.push("Dynamically loaded assets may still be used.");
  }

  return lines.join("\n");
}

function assetUsageCount(asset) {
  return Number.isInteger(asset.usageCount)
    ? asset.usageCount
    : Array.isArray(asset.references)
      ? asset.references.length
      : 0;
}

module.exports = {
  AssetTreeDataProvider,
};
