let direct = false;
let indirect = false;
let generated = false;
let alias = eval;

try {
    eval("@");
} catch (error) {
    direct = error instanceof SyntaxError && error.name === "SyntaxError";
}

try {
    alias("break missingLabel");
} catch (error) {
    indirect = error instanceof SyntaxError && error.name === "SyntaxError";
}

try {
    Function("}");
} catch (error) {
    generated = error instanceof SyntaxError && error.name === "SyntaxError";
}

print(direct, indirect, generated);

direct && indirect && generated ? 42 : 0
