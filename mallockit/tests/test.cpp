#include <array>
#include <iostream>

using namespace std;

template <size_t M, size_t N>
using Matrix = array<array<int, N>, M>;

template <size_t M, size_t N, size_t P>
constexpr Matrix<M, P> operator*(Matrix<M, N> A, Matrix<N, P> B)
{
    Matrix<M, P> C{};
    for (int i = 0; i < M; i++)
        for (int k = 0; k < P; k++)
            for (int j = 0; j < N; j++)
                C[i][k] += A[i][j] * B[j][k];
    return C;
}

int main()
{
    constexpr Matrix<2, 3> A = {{
        {1, 2, 3},
        {4, 5, 6},
    }};
    constexpr Matrix<3, 2> B = {{
        {1, 2},
        {3, 4},
        {5, 6},
    }};
    auto C = A * B;
    for (auto r : C)
    {
        for (auto x : r)
            cout << x << " ";
        cout << "\n";
    }
}