<?php

namespace App;

class CrossFileChild extends CrossFileBase
{
}

function use_cross_file_inheritance(): float
{
    return 1 / CrossFileChild::zero();
}
