<?php

namespace App;

class SparseMethodExample
{
    public function sparseMethod(): int
    {
        $x = 1;
        $y = 2;
        $z = $x + $y;
        $result = $z * 3;

        return $result;
    }
}
