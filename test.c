#include <stdio.h>
#include <stdlib.h>

// extern void bar();

int main() {
    // bar();
    // while (1) {
        void* p = malloc(233);
        printf("hello %p \n", p);
    // }
}