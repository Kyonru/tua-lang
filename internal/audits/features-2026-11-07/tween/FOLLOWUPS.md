# Follow-ups

- Common shortened-callback arity defect is tracked centrally (STORES-01). Tween callbacks have zero formal runtime arguments, so this pass does not independently reproduce or count that issue. The typed callback probe instead confirms that a callback requiring one number is rejected correctly.
- Decide whether to fix inherited flux cancellation behavior or explicitly document callback cancellation restrictions; any runtime change should update the unchanged-upstream claim.
- Delay overshoot is inherited frame-based behavior: update(1) after delay(0.25) consumes the entire frame waiting. Consider documenting timing semantics if users expect elapsed-time conservation; not counted as a defect under the stated flux compatibility contract.
