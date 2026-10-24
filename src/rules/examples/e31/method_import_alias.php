<?php

namespace Vendor {
    class ImportedBase
    {
        public static function zero(): int
        {
            return 0;
        }
    }
}

namespace App {
    use Vendor\ImportedBase as Base;

    class ImportedChild extends Base
    {
    }

    function use_imported_child(): float
    {
        return 1 / ImportedChild::zero();
    }
}
