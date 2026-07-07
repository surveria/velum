let total = 0;

for (let index = 0; index < 8192; index = index + 1) {
    try {
        if ((index & 3) === 0) {
            throw 1;
        }
        total = total + 1;
    } catch (error) {
        total = total + error;
    } finally {
        total = total + 1;
    }
}

total;
