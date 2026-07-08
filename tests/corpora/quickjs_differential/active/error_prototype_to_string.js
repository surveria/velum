let error = new TypeError("typed");
let aggregate = new AggregateError([], "many");

print(error.toString());
print(Object.getPrototypeOf(error) === TypeError.prototype);
print(Object.getPrototypeOf(TypeError.prototype) === Error.prototype);
print(TypeError.prototype.toString === Error.prototype.toString);
print(error instanceof Error, error instanceof TypeError, error instanceof SyntaxError);
print(Error.prototype.toString.call({ name: "Custom", message: "message" }));
print(Error.prototype.toString.call({ name: "", message: "message" }));
print(Error.prototype.toString.call({ name: "OnlyName", message: "" }));
print(aggregate.name, aggregate.message, AggregateError.length);
