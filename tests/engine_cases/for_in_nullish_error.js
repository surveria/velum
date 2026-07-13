let count = 0;
for (let key in null) {
    count = count + 1;
}
for (let key in undefined) {
    count = count + 1;
}
count === 0 ? 42 : 0
