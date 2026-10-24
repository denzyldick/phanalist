<?php

namespace Vendor\Nested {
    class ImportedBase
    {
        public static function zero(): int
        {
            return 0;
        }
    }
}

namespace App {
    use Vendor\Nested\{ImportedBase as Base};

    class ImportedChild extends Base
    {
    }

    function use_group_import(): float
    {
        return 1 / ImportedChild::zero();
    }
}
