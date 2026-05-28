<?php

namespace App;

use App\Service\A;
use App\Service\B;
use App\Service\C;
use App\Service\D;
use App\Service\E;
use App\Service\F;
use App\Service\G;
use App\Service\H;
use App\Service\I;
use App\Service\J;
use App\Service\K;
use App\Service\L;

class HighFanOut
{
    public function __construct(
        private A $a,
        private B $b,
        private C $c,
        private D $d,
        private E $e,
        private F $f,
        private G $g,
        private H $h,
        private I $i,
        private J $j,
        private K $k,
        private L $l,
    ) {}

    public function execute(): void
    {
        $m = new \App\Service\M();
        $result = $m instanceof \App\Service\N;
        \App\Service\O::staticMethod();
    }
}
