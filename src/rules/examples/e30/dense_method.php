<?php

namespace App;

class DenseMethodExample
{
    public function denseMethod($a, $b, $c): int
    {
        if ($a) { if ($b) { return 1; } else { if ($c) { return 2; } } }
        if ($a && $b || $c) { return 3; }
        if (!$a) { return 4; }

        return 0;
    }
}
