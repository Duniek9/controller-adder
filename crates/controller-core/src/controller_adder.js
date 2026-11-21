/*:
 * @plugindesc Controller Adder generated native gamepad mapping.
 * @help Load after other input plugins. Disable to restore original handling.
 * Uses standard Gamepad API layout. Does not synthesize keyboard events.
 */
(function() {
    "use strict";
    var bindings = JSON.parse(__CONTROLLER_ADDER_BINDINGS__);
    if (typeof Input === "undefined" || typeof Input._pollGamepads !== "function") {
        throw new Error("Controller Adder: unsupported Input implementation");
    }
    if (Input._controllerAdderInstalled) return;
    Input._controllerAdderInstalled = true;

    // Remove our overlay before keyboard handlers or the next input update,
    // restoring the underlying state instead of clearing keyboard-held actions.
    var overlay = Object.create(null);
    function restore(input) {
        Object.keys(overlay).forEach(function(action) {
            var entry = overlay[action];
            if (entry.state !== input._currentState) return;
            if (entry.present) input._currentState[action] = entry.value;
            else delete input._currentState[action];
        });
        overlay = Object.create(null);
    }

    function active(pad, binding) {
        if (binding.kind === "button") {
            var b = pad.buttons[binding.index];
            return typeof b === "number" ? b >= 0.5 :
                !!b && (b.pressed || b.value >= 0.5);
        }
        var value = pad.axes[binding.index];
        return typeof value === "number" && isFinite(value) &&
            (binding.direction === "negative" ? -value : value) >= binding.threshold;
    }

    var clear = Input.clear;
    Input.clear = function() {
        restore(this);
        return clear.apply(this, arguments);
    };

    // Keep engine/plugin keyboard handling intact; reapply active controller
    // actions immediately afterward so releasing one source cannot cancel another.
    ["_onKeyDown", "_onKeyUp"].forEach(function(name) {
        var previous = Input[name];
        if (typeof previous !== "function") return;
        Input[name] = function() {
            var held = Object.keys(overlay);
            restore(this);
            try { return previous.apply(this, arguments); }
            finally {
                var self = this;
                held.forEach(function(action) { apply(self, action); });
            }
        };
    });

    function apply(input, action) {
        var state = input._currentState;
        overlay[action] = {
            state: state,
            present: Object.prototype.hasOwnProperty.call(state, action),
            value: state[action]
        };
        state[action] = true;
    }

    // Replace the stock poller rather than allowing its default button mapping
    // and built-in axis-to-D-pad conversion to emit competing actions.
    // The stock Input.update still handles repeat, trigger and direction logic.
    Input._pollGamepads = function() {
        restore(this);
        var pads = navigator.getGamepads ? navigator.getGamepads() : [];
        var self = this;
        Object.keys(bindings).forEach(function(action) {
            for (var i = 0; pads && i < pads.length; i++) {
                var pad = pads[i];
                if (!pad || !pad.connected || pad.mapping !== "standard") continue;
                if (bindings[action].some(function(binding) { return active(pad, binding); })) {
                    apply(self, action);
                    break;
                }
            }
        });
    };
})();
