const windows = workspace.windowList().filter(window => window.caption === "TERAMIC");
if (windows.length === 1) {
    const window = windows[0];
    workspace.activeWindow = window;
    print("TERAMIC_POINTER " + JSON.stringify({stamp: Date.now(), client: window.clientGeometry, cursor: workspace.cursorPos}));
} else {
    print("TERAMIC_POINTER " + JSON.stringify({stamp: Date.now(), error: "Expected one TERAMIC window"}));
}
